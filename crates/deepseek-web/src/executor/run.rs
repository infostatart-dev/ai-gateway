use std::{future::Future, pin::Pin, sync::Arc};

use serde_json::Value;
use tracing::Instrument;
use web_message_budget::WebTurnKind;
use web_structured_output::{
    StructuredOutputIssue, StructuredOutputMode, base_system_without_schema,
    build_json_object_instruction, build_schema_instruction,
    check_structured_response, parse_json_schema_spec, retry_suffix_for,
    structured_output_mode,
};

use super::turn::{TurnContext, run_completion_turn};
use crate::{
    Error,
    api::{create_session_with_browser, delete_session_with_browser},
    completion::{
        parse_openai_messages, plan_completion_turns, web_turn_to_prompt,
    },
    pow::cache::PowCache,
    session::{
        exchange::{exchange_browser_session, invalidate_token_cache},
        file::BrowserSession,
    },
    sse::{build_non_stream_response, collect_sse, transform_sse_to_openai},
    tls::fetch::{HttpFetch, default_fetch},
};

const MAX_STRUCTURED_RETRIES: u32 = 2;

pub type TurnHook = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>
        + Send
        + Sync,
>;

#[derive(Debug, Clone, Default)]
pub struct ExecuteStats {
    pub turns: u32,
    pub upload_parts: u32,
    pub pow_cache_hits: u32,
}

#[derive(Clone)]
pub struct ExecuteRequest {
    pub user_token: String,
    pub browser_session: Option<BrowserSession>,
    pub body: Value,
    pub stream: bool,
    pub turn_hook: Option<TurnHook>,
}

impl std::fmt::Debug for ExecuteRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecuteRequest")
            .field("user_token", &"<redacted>")
            .field("browser_session", &self.browser_session.is_some())
            .field("body", &self.body)
            .field("stream", &self.stream)
            .field("turn_hook", &self.turn_hook.is_some())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct ExecuteResult {
    pub status: u16,
    pub body: Vec<u8>,
    pub stats: ExecuteStats,
}

pub struct Executor {
    fetch: Arc<dyn HttpFetch>,
}

impl Default for Executor {
    fn default() -> Self {
        Self {
            fetch: default_fetch(),
        }
    }
}

impl Executor {
    pub fn new(fetch: Arc<dyn HttpFetch>) -> Self {
        Self { fetch }
    }

    pub async fn execute(
        &self,
        req: ExecuteRequest,
    ) -> Result<ExecuteResult, Error> {
        let model = req
            .body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("deepseek-chat")
            .to_string();
        let messages = req
            .body
            .get("messages")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if messages.is_empty() {
            return Ok(error_body(400, "Missing or empty messages array"));
        }

        let mode = structured_output_mode(&req.body);
        let schema_spec = parse_json_schema_spec(&req.body);
        let structured_required = mode.is_some();
        let reserved_output = req
            .body
            .get("max_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(4_096) as u32;

        let user_token =
            crate::session::token::normalize_user_token(&req.user_token);
        let browser_session = req
            .browser_session
            .clone()
            .unwrap_or_else(|| BrowserSession::from_token(user_token.clone()));
        let (access, session_id) = async {
            let access =
                exchange_browser_session(self.fetch.as_ref(), &browser_session)
                    .await?;
            let session_id = create_session_with_browser(
                self.fetch.as_ref(),
                &access.token,
                Some(&browser_session),
            )
            .await?;
            Ok::<_, Error>((access, session_id))
        }
        .instrument(tracing::info_span!("gateway.web.deepseek.session_prepare"))
        .await?;
        let pow_cache = PowCache::new();

        let mut structured_attempt = 0u32;
        let mut last_issue = None;

        let result = loop {
            let retry_suffix = structured_attempt
                .gt(&0)
                .then(|| retry_suffix_for(last_issue));
            match self
                .run_planned_turns(
                    &req,
                    &model,
                    &access.token,
                    &session_id,
                    &browser_session,
                    &pow_cache,
                    schema_spec.as_ref(),
                    mode,
                    reserved_output,
                    retry_suffix,
                )
                .await
            {
                Ok((response, _raw_sse, stats))
                    if structured_required && !req.stream =>
                {
                    let issue = check_structured_response(
                        &response,
                        schema_spec.as_ref(),
                    );
                    if let Some(i) = issue {
                        last_issue = Some(i);
                        if structured_attempt < MAX_STRUCTURED_RETRIES {
                            structured_attempt += 1;
                            continue;
                        }
                        break Ok(error_body(
                            502,
                            structured_failure_message(i),
                        ));
                    }
                    break Ok(ExecuteResult {
                        status: 200,
                        body: serde_json::to_vec(&response)?,
                        stats,
                    });
                }
                Ok((response, raw_sse, stats)) if req.stream => {
                    let body = raw_sse
                        .map(|raw| transform_sse_to_openai(&raw, &model))
                        .unwrap_or_else(|| {
                            serde_json::to_vec(&response).unwrap_or_default()
                        });
                    break Ok(ExecuteResult {
                        status: 200,
                        body,
                        stats,
                    });
                }
                Ok((response, _, stats)) => {
                    break Ok(ExecuteResult {
                        status: 200,
                        body: serde_json::to_vec(&response)?,
                        stats,
                    });
                }
                Err(e @ Error::CredentialRestricted { .. }) => break Err(e),
                Err(e) => break Err(e),
            }
        };

        delete_session_with_browser(
            self.fetch.as_ref(),
            &access.token,
            &session_id,
            Some(&browser_session),
        )
        .await;

        match result {
            Ok(ok) => Ok(ok),
            Err(e) => {
                if matches!(e, Error::SessionAuth(_)) {
                    invalidate_token_cache(&user_token);
                }
                Err(e)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_planned_turns(
        &self,
        req: &ExecuteRequest,
        model: &str,
        access_token: &str,
        session_id: &str,
        browser_session: &BrowserSession,
        pow_cache: &PowCache,
        schema_spec: Option<&web_structured_output::JsonSchemaSpec>,
        mode: Option<StructuredOutputMode>,
        reserved_output: u32,
        retry_suffix: Option<&str>,
    ) -> Result<(Value, Option<String>, ExecuteStats), Error> {
        let parsed = parse_openai_messages(
            req.body
                .get("messages")
                .and_then(Value::as_array)
                .map(|a| a.as_slice())
                .unwrap_or(&[]),
        );
        let schema_instruction = match mode {
            Some(StructuredOutputMode::JsonSchema) => {
                schema_spec.map(build_schema_instruction)
            }
            Some(StructuredOutputMode::JsonObject) => {
                Some(build_json_object_instruction())
            }
            None => None,
        };
        let base_system = base_system_without_schema(
            &parsed.system_msg,
            schema_instruction.as_deref(),
        );
        let plan = plan_completion_turns(
            &parsed,
            &base_system,
            schema_instruction.as_deref(),
            reserved_output,
        );

        let upload_parts = plan
            .turns
            .iter()
            .filter(|t| matches!(t.kind, WebTurnKind::ContextUpload { .. }))
            .count() as u32;
        let mut stats = ExecuteStats {
            upload_parts,
            ..ExecuteStats::default()
        };

        let turn_ctx = TurnContext {
            fetch: self.fetch.as_ref(),
            access_token,
            session_id,
            model,
            body: &req.body,
            pow_cache,
            browser_session: Some(browser_session),
        };

        let turn_count = plan.turns.len();
        let mut last_raw_sse = None;
        let mut last_collected = None;

        for (idx, turn) in plan.turns.iter().enumerate() {
            stats.turns += 1;

            let mut prompt = web_turn_to_prompt(turn);
            let is_final = idx + 1 == turn_count;
            let is_upload =
                matches!(turn.kind, WebTurnKind::ContextUpload { .. });
            let turn_kind = if is_upload {
                "upload"
            } else if is_final {
                "final"
            } else {
                "turn"
            };
            if is_final && let Some(suffix) = retry_suffix {
                prompt.push_str(suffix);
            }

            let turn_span = tracing::info_span!(
                "gateway.web.deepseek.turn",
                turn_index = idx,
                turns = turn_count,
                final = is_final,
                kind = turn_kind,
            );
            let run_turn = async {
                if let Some(hook) = &req.turn_hook {
                    hook().await?;
                }
                run_completion_turn(&turn_ctx, prompt).await
            };
            let raw_sse = if is_upload {
                run_turn
                    .instrument(tracing::info_span!(
                        "gateway.web.deepseek.upload",
                        turn_index = idx,
                    ))
                    .instrument(turn_span)
                    .await?
            } else {
                run_turn.instrument(turn_span).await?
            };
            let collected = collect_sse(&raw_sse, model);

            if is_final {
                if collected.content.is_empty()
                    && collected.reasoning_content.is_empty()
                {
                    return Err(Error::EmptyResponse);
                }
                last_raw_sse = Some(raw_sse);
                last_collected = Some(collected);
            } else if collected.content.trim().is_empty()
                && collected.reasoning_content.trim().is_empty()
            {
                return Err(Error::EmptyResponse);
            }
        }

        stats.pow_cache_hits = pow_cache.cache_hits();
        let collected = last_collected.ok_or(Error::EmptyResponse)?;
        Ok(
            tracing::info_span!("gateway.web.deepseek.response_finalize")
                .in_scope(|| {
                    (
                        build_non_stream_response(model, &collected),
                        last_raw_sse,
                        stats,
                    )
                }),
        )
    }
}

fn structured_failure_message(issue: StructuredOutputIssue) -> &'static str {
    match issue {
        StructuredOutputIssue::InvalidJson => {
            "DeepSeek response was not valid JSON after retries"
        }
        StructuredOutputIssue::SchemaMismatch => {
            "DeepSeek response did not match the required JSON schema after \
             retries"
        }
    }
}

fn error_body(status: u16, message: &str) -> ExecuteResult {
    ExecuteResult {
        status,
        body: serde_json::json!({
            "error": { "message": message, "type": "upstream_error" }
        })
        .to_string()
        .into_bytes(),
        stats: ExecuteStats::default(),
    }
}

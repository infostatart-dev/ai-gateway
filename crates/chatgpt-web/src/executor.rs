use std::sync::Arc;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    Error,
    constants::{CONV_URL, JSON_RETRY_SUFFIX, SCHEMA_RETRY_SUFFIX},
    conversation::{
        build_conversation_body, build_non_streaming_response,
        collect_sse_turn_meta, parse_openai_messages, plan_conversation_turns,
    },
    headers::{browser_headers, oai_headers},
    models::map_model,
    schema::{
        StructuredOutputIssue, base_system_without_schema,
        build_schema_instruction, check_structured_response,
        parse_json_schema_spec,
    },
    sentinel::{
        dpl::{build_prekey_config, fallback_dpl, fetch_dpl},
        pow::solve_proof_of_work,
        prepare::{PrepareChatInput, prepare_chat_requirements},
    },
    session::{
        cookie::build_session_cookie_header,
        exchange::{exchange_session, invalidate_token_cache},
        file::{SessionFile, save_session},
        warmup::run_session_warmup,
    },
    tls::fetch::{FetchRequest, HttpFetch, default_fetch},
};

const MAX_STRUCTURED_RETRIES: u32 = 2;

#[derive(Debug, Clone)]
pub struct ExecuteRequest {
    pub cookie: String,
    pub body: Value,
    pub json_schema_required: bool,
    pub session_path: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ExecuteResult {
    pub status: u16,
    pub body: Vec<u8>,
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
            .unwrap_or("gpt-5-mini")
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

        let schema_spec = parse_json_schema_spec(&req.body);
        let structured_required =
            req.json_schema_required || schema_spec.is_some();
        let mut cookie = req.cookie.clone();
        let mut structured_attempt = 0u32;
        let mut last_issue = None;

        let reserved_output = req
            .body
            .get("max_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(4_096) as u32;

        loop {
            let retry_suffix = structured_attempt
                .gt(&0)
                .then(|| retry_suffix_for(last_issue));

            match self
                .execute_once(
                    &cookie,
                    &model,
                    &req.body,
                    &messages,
                    retry_suffix,
                    reserved_output,
                )
                .await
            {
                Ok((new_cookie, response)) => {
                    if new_cookie != cookie {
                        cookie = new_cookie.clone();
                        if let Some(path) = &req.session_path {
                            let _ = save_session(
                                path,
                                &SessionFile {
                                    cookie: new_cookie,
                                    saved_at: chrono::Utc::now(),
                                },
                            )
                            .await;
                        }
                    }

                    if structured_required
                        && let Some(issue) = check_structured_response(
                            &response,
                            schema_spec.as_ref(),
                        )
                    {
                        last_issue = Some(issue);
                        if structured_attempt < MAX_STRUCTURED_RETRIES {
                            structured_attempt += 1;
                            continue;
                        }
                        return Ok(error_body(
                            502,
                            structured_failure_message(issue),
                        ));
                    }

                    return Ok(ExecuteResult {
                        status: 200,
                        body: serde_json::to_vec(&response)?,
                    });
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn execute_once(
        &self,
        cookie: &str,
        model: &str,
        body: &Value,
        messages: &[Value],
        retry_suffix: Option<&'static str>,
        reserved_output_tokens: u32,
    ) -> Result<(String, Value), Error> {
        let token = exchange_session(self.fetch.as_ref(), cookie).await?;
        let cookie = token
            .refreshed_cookie
            .clone()
            .unwrap_or_else(|| cookie.to_string());

        let (dpl, script_src) =
            match fetch_dpl(self.fetch.as_ref(), &cookie).await {
                Ok(v) => v,
                Err(_) => fallback_dpl(),
            };

        let session_id = uuid::Uuid::new_v4().to_string();
        let device_id = device_id_for(&cookie);

        run_session_warmup(
            self.fetch.as_ref(),
            &token.access_token,
            token.account_id.as_deref(),
            &session_id,
            &device_id,
            &cookie,
        )
        .await;

        let reqs = prepare_chat_requirements(
            self.fetch.as_ref(),
            PrepareChatInput {
                access_token: &token.access_token,
                account_id: token.account_id.as_deref(),
                session_id: &session_id,
                device_id: &device_id,
                cookie: &cookie,
                dpl: &dpl,
                script_src: &script_src,
            },
        )
        .await?;

        let proof_token = if reqs
            .proofofwork
            .as_ref()
            .is_some_and(|p| p.required.unwrap_or(false))
        {
            let pow = reqs.proofofwork.as_ref().unwrap();
            if let (Some(seed), Some(diff)) = (&pow.seed, &pow.difficulty) {
                let config = build_prekey_config(
                    crate::constants::CHATGPT_USER_AGENT,
                    &dpl,
                    &script_src,
                );
                Some(
                    tokio::task::spawn_blocking({
                        let seed = seed.clone();
                        let diff = diff.clone();
                        move || solve_proof_of_work(&seed, &diff, config)
                    })
                    .await
                    .map_err(|e| Error::Other(e.to_string()))?,
                )
            } else {
                None
            }
        } else {
            None
        };

        let parsed = parse_openai_messages(messages);
        let schema_spec = parse_json_schema_spec(body);
        let schema_instruction =
            schema_spec.as_ref().map(build_schema_instruction);
        let base_system = base_system_without_schema(
            &parsed.system_msg,
            schema_instruction.as_deref(),
        );

        let plan = plan_conversation_turns(
            &parsed,
            &base_system,
            schema_instruction.as_deref(),
            reserved_output_tokens,
        );

        if plan.turns.is_empty()
            || (plan.turns.len() == 1
                && plan.turns[0].user_msg.trim().is_empty()
                && plan.turns[0].system_msg.trim().is_empty())
        {
            return Ok((cookie, serde_json::json!({})));
        }

        let model_slug = map_model(model);
        let mut conversation_id: Option<String> = None;
        let mut parent_message_id = uuid::Uuid::new_v4().to_string();
        let turn_count = plan.turns.len();

        for (idx, turn) in plan.turns.iter().enumerate() {
            let is_final = idx + 1 == turn_count;
            let mut system_msg = turn.system_msg.clone();
            if is_final && let Some(suffix) = retry_suffix {
                system_msg.push_str(suffix);
            }
            let turn_parsed = web_message_budget::ParsedChat {
                system_msg,
                history: vec![],
                current_msg: turn.user_msg.clone(),
            };
            let cgpt_body = build_conversation_body(
                &turn_parsed,
                &model_slug,
                &parent_message_id,
                conversation_id.as_deref(),
            );

            let resp = self
                .fetch
                .as_ref()
                .fetch(FetchRequest {
                    url: CONV_URL.into(),
                    method: "POST".into(),
                    headers: conv_headers(
                        &token,
                        &session_id,
                        &device_id,
                        &cookie,
                        &reqs,
                        proof_token.as_deref(),
                    ),
                    body: Some(serde_json::to_vec(&cgpt_body)?),
                    timeout_ms: 120_000,
                })
                .await?;

            if resp.status == 401 || resp.status == 403 {
                invalidate_token_cache(&cookie);
                return Err(Error::SessionAuth(
                    "conversation unauthorized".into(),
                ));
            }
            if resp.status == 429 {
                return Err(Error::Upstream {
                    status: 429,
                    message: "rate limited".into(),
                });
            }
            if resp.status >= 400 {
                return Err(Error::Upstream {
                    status: resp.status,
                    message: String::from_utf8_lossy(&resp.body).into(),
                });
            }
            if resp.body.is_empty() {
                return Err(Error::EmptyResponse);
            }

            let raw = String::from_utf8_lossy(&resp.body);
            let meta = collect_sse_turn_meta(&raw).map_err(Error::Other)?;
            if is_final {
                if meta.content.trim().is_empty() {
                    return Err(Error::EmptyResponse);
                }
                return Ok((
                    cookie,
                    build_non_streaming_response(model, &meta.content),
                ));
            }
            conversation_id = meta
                .conversation_id
                .or(conversation_id)
                .or_else(|| {
                    cgpt_body
                        .get("conversation_id")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                });
            parent_message_id = meta.assistant_message_id.ok_or_else(|| {
                Error::Other(
                    "missing assistant message id for context upload turn"
                        .into(),
                )
            })?;
        }

        Err(Error::Other("chunk plan produced no final turn".into()))
    }
}

fn conv_headers(
    token: &crate::session::exchange::TokenEntry,
    session_id: &str,
    device_id: &str,
    cookie: &str,
    reqs: &crate::sentinel::prepare::ChatRequirements,
    proof_token: Option<&str>,
) -> Vec<(String, String)> {
    let mut headers = browser_headers();
    headers.extend(oai_headers(session_id, device_id));
    headers.push(("Content-Type".into(), "application/json".into()));
    headers.push(("Accept".into(), "text/event-stream".into()));
    headers.push((
        "Authorization".into(),
        format!("Bearer {}", token.access_token),
    ));
    headers.push(("Cookie".into(), build_session_cookie_header(cookie)));
    if let Some(id) = &token.account_id {
        headers.push(("chatgpt-account-id".into(), id.clone()));
    }
    if let Some(t) = &reqs.token {
        headers.push((
            "openai-sentinel-chat-requirements-token".into(),
            t.clone(),
        ));
    }
    if let Some(t) = &reqs.prepare_token {
        headers.push((
            "openai-sentinel-chat-requirements-prepare-token".into(),
            t.clone(),
        ));
    }
    if let Some(t) = proof_token {
        headers.push(("openai-sentinel-proof-token".into(), t.to_string()));
    }
    headers
}

fn retry_suffix_for(issue: Option<StructuredOutputIssue>) -> &'static str {
    match issue {
        Some(StructuredOutputIssue::SchemaMismatch) => SCHEMA_RETRY_SUFFIX,
        Some(StructuredOutputIssue::InvalidJson) | None => JSON_RETRY_SUFFIX,
    }
}

fn structured_failure_message(issue: StructuredOutputIssue) -> &'static str {
    match issue {
        StructuredOutputIssue::InvalidJson => {
            "ChatGPT response was not valid JSON after retries"
        }
        StructuredOutputIssue::SchemaMismatch => {
            "ChatGPT response did not match the required JSON schema after \
             retries"
        }
    }
}

fn device_id_for(cookie: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(cookie.as_bytes());
    let h: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!(
        "{}-{}-4{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[13..16],
        &h[16..20],
        &h[20..32]
    )
}

fn error_body(status: u16, message: &str) -> ExecuteResult {
    ExecuteResult {
        status,
        body: serde_json::json!({
            "error": { "message": message, "type": "upstream_error" }
        })
        .to_string()
        .into_bytes(),
    }
}

#[cfg(test)]
mod tests;

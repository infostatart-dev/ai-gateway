use std::sync::Arc;

use serde_json::Value;

use crate::{
    Error,
    api::{create_pow_challenge, create_session, delete_session},
    completion::{
        build_completion_request, completion_headers, completion_json,
    },
    constants::COMPLETION_URL,
    pow::solve_challenge,
    session::exchange::{exchange_session, invalidate_token_cache},
    sse::{build_non_stream_response, collect_sse, transform_sse_to_openai},
    tls::fetch::{FetchRequest, HttpFetch, default_fetch},
};

#[derive(Debug, Clone)]
pub struct ExecuteRequest {
    pub user_token: String,
    pub body: Value,
    pub stream: bool,
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

        let user_token =
            crate::session::token::normalize_user_token(&req.user_token);
        let access = exchange_session(self.fetch.as_ref(), &user_token).await?;
        let session_id =
            create_session(self.fetch.as_ref(), &access.token).await?;

        let result = self
            .perform_completion(&access.token, &model, &req.body, &session_id)
            .await;

        delete_session(self.fetch.as_ref(), &access.token, &session_id).await;

        match result {
            Ok(raw_sse) => {
                if req.stream {
                    Ok(ExecuteResult {
                        status: 200,
                        body: transform_sse_to_openai(&raw_sse, &model),
                    })
                } else {
                    let collected = collect_sse(&raw_sse, &model);
                    if collected.content.is_empty()
                        && collected.reasoning_content.is_empty()
                    {
                        return Err(Error::EmptyResponse);
                    }
                    Ok(ExecuteResult {
                        status: 200,
                        body: serde_json::to_vec(&build_non_stream_response(
                            &model, &collected,
                        ))?,
                    })
                }
            }
            Err(e) => {
                if matches!(e, Error::SessionAuth(_)) {
                    invalidate_token_cache(&user_token);
                }
                Err(e)
            }
        }
    }

    async fn perform_completion(
        &self,
        access_token: &str,
        model: &str,
        body: &Value,
        session_id: &str,
    ) -> Result<String, Error> {
        let challenge =
            create_pow_challenge(self.fetch.as_ref(), access_token).await?;
        let pow_answer = solve_challenge(&challenge)?;
        let completion = build_completion_request(body, model, session_id, 0);
        let headers = completion_headers(access_token, &pow_answer);
        let payload = completion_json(&completion);

        let resp = self
            .fetch
            .as_ref()
            .fetch(FetchRequest {
                url: COMPLETION_URL.into(),
                method: "POST".into(),
                headers,
                body: Some(serde_json::to_vec(&payload)?),
                timeout_ms: 120_000,
            })
            .await?;

        if resp.status == 401 || resp.status == 403 {
            return Err(Error::SessionAuth(
                "DeepSeek token expired — get a fresh userToken from \
                 localStorage"
                    .into(),
            ));
        }
        if resp.status == 429 {
            return Err(Error::Upstream {
                status: 429,
                message: "DeepSeek rate limited. Wait and retry.".into(),
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

        if resp
            .header("content-type")
            .is_some_and(|ct| ct.contains("application/json"))
            && let Ok(v) = serde_json::from_slice::<Value>(&resp.body)
            && let Some(code) = v.get("code").and_then(Value::as_i64)
            && code != 0
        {
            let msg = v
                .get("msg")
                .and_then(Value::as_str)
                .unwrap_or("DeepSeek error");
            return Err(Error::Upstream {
                status: map_ds_code(code),
                message: format!("DeepSeek error {code}: {msg}"),
            });
        }

        Ok(String::from_utf8_lossy(&resp.body).into())
    }
}

fn map_ds_code(code: i64) -> u16 {
    match code {
        40003 => 401,
        40002 => 429,
        _ => 502,
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
    }
}

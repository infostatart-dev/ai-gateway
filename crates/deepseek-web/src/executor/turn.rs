use serde_json::Value;

use crate::{
    Error,
    api::create_pow_challenge,
    completion::{
        build_completion_from_prompt, completion_headers, completion_json,
    },
    constants::COMPLETION_URL,
    pow::{cache::PowCache, solve_challenge},
    tls::fetch::{FetchRequest, HttpFetch},
};

pub struct TurnContext<'a> {
    pub fetch: &'a dyn HttpFetch,
    pub access_token: &'a str,
    pub session_id: &'a str,
    pub model: &'a str,
    pub body: &'a Value,
    pub pow_cache: &'a PowCache,
}

pub async fn run_completion_turn(
    ctx: &TurnContext<'_>,
    prompt: String,
) -> Result<String, Error> {
    let challenge = create_pow_challenge(ctx.fetch, ctx.access_token).await?;
    let pow_answer = if let Some(cached) =
        ctx.pow_cache
            .get(ctx.access_token, ctx.session_id, &challenge)
    {
        cached
    } else {
        let solved = solve_challenge(&challenge)?;
        ctx.pow_cache.store(
            ctx.access_token,
            ctx.session_id,
            challenge,
            solved.clone(),
        );
        solved
    };

    let completion = build_completion_from_prompt(
        ctx.body,
        ctx.model,
        ctx.session_id,
        prompt,
    );
    let headers = completion_headers(ctx.access_token, &pow_answer);
    let payload = completion_json(&completion);

    let resp = ctx
        .fetch
        .fetch(FetchRequest {
            url: COMPLETION_URL.into(),
            method: "POST".into(),
            headers,
            body: Some(serde_json::to_vec(&payload)?),
            timeout_ms: 120_000,
        })
        .await?;

    if resp.status == 401 || resp.status == 403 {
        ctx.pow_cache.invalidate(ctx.access_token, ctx.session_id);
        return Err(Error::SessionAuth(
            "DeepSeek token expired — get a fresh userToken from localStorage"
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
        ctx.pow_cache.invalidate(ctx.access_token, ctx.session_id);
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
        && let Some(err) =
            crate::biz_error::parse_completion_json_error(&resp.body)
    {
        ctx.pow_cache.invalidate(ctx.access_token, ctx.session_id);
        return Err(err);
    }

    if resp
        .header("content-type")
        .is_some_and(|ct| ct.contains("application/json"))
        && let Ok(v) = serde_json::from_slice::<Value>(&resp.body)
        && let Some(code) = v.get("code").and_then(Value::as_i64)
        && code != 0
    {
        ctx.pow_cache.invalidate(ctx.access_token, ctx.session_id);
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

fn map_ds_code(code: i64) -> u16 {
    match code {
        40003 => 401,
        40002 => 429,
        _ => 502,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{
        pow::cache::PowCache,
        tls::fetch::{FetchResponse, MockFetch},
    };

    fn json_response(body: serde_json::Value) -> FetchResponse {
        FetchResponse {
            status: 200,
            headers: vec![("content-type".into(), "application/json".into())],
            body: body.to_string().into_bytes(),
        }
    }

    fn pow_challenge_response() -> FetchResponse {
        json_response(json!({
            "code": 0,
            "data": { "biz_data": { "challenge": {
                "algorithm": "DeepSeekHashV1",
                "challenge": "d1052c4a04fb634e3ac66d36bfeaa583d769839823812090d679b23de6048d6d",
                "salt": "abc",
                "signature": "sig",
                "difficulty": 1000,
                "expire_at": 1_234_567_890_i64,
                "target_path": "/api/v0/chat/completion"
            }}}
        }))
    }

    #[tokio::test]
    async fn mute_json_is_credential_restricted_not_empty_response() {
        let mute = json!({
            "code": 0,
            "data": {
                "biz_code": 5,
                "biz_msg": "user is muted",
                "biz_data": { "mute_until": 1781861651.742 }
            }
        });
        let fetch =
            MockFetch::new(vec![pow_challenge_response(), json_response(mute)]);
        let body = json!({ "model": "deepseek-chat" });
        let pow_cache = PowCache::new();
        let ctx = TurnContext {
            fetch: fetch.as_ref(),
            access_token: "access-token",
            session_id: "session-id",
            model: "deepseek-chat",
            body: &body,
            pow_cache: &pow_cache,
        };

        let err = run_completion_turn(&ctx, "hello".into()).await.unwrap_err();
        match err {
            Error::CredentialRestricted {
                message,
                restricted_until,
            } => {
                assert_eq!(message, "user is muted");
                assert_eq!(restricted_until, Some(1_781_861_651));
            }
            other => panic!("expected CredentialRestricted, got {other:?}"),
        }
        assert_eq!(fetch.call_count(), 2, "single PoW + completion attempt");
    }
}

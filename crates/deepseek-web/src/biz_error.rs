use serde_json::Value;

use crate::Error;

const BIZ_CODE_USER_MUTED: i64 = 5;

#[must_use]
pub fn parse_completion_json_error(body: &[u8]) -> Option<Error> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let top_code = value.get("code").and_then(Value::as_i64).unwrap_or(0);
    let data = value
        .get("data")
        .filter(|v| v.is_object())
        .or_else(|| value.get("biz_data").filter(|v| v.is_object()));

    if let Some(data) = data {
        let biz_code =
            data.get("biz_code").and_then(Value::as_i64).unwrap_or(0);
        if biz_code != 0 {
            return Some(map_biz_code(biz_code, data, &value));
        }
    }

    if top_code != 0 {
        let msg = value
            .get("msg")
            .and_then(Value::as_str)
            .unwrap_or("DeepSeek error");
        return Some(Error::Upstream {
            status: map_ds_code(top_code),
            message: format!("DeepSeek error {top_code}: {msg}"),
        });
    }

    None
}

fn map_biz_code(biz_code: i64, data: &Value, root: &Value) -> Error {
    match biz_code {
        BIZ_CODE_USER_MUTED => {
            let message = data
                .get("biz_msg")
                .and_then(Value::as_str)
                .or_else(|| root.get("msg").and_then(Value::as_str))
                .unwrap_or("user is muted")
                .to_string();
            let restricted_until = data
                .get("biz_data")
                .and_then(|biz| biz.get("mute_until"))
                .and_then(parse_unix_timestamp);
            Error::CredentialRestricted {
                message,
                restricted_until,
            }
        }
        other => {
            let msg = data
                .get("biz_msg")
                .and_then(Value::as_str)
                .unwrap_or("DeepSeek error");
            Error::Upstream {
                status: map_ds_biz_code(other),
                message: format!("DeepSeek biz error {other}: {msg}"),
            }
        }
    }
}

fn parse_unix_timestamp(value: &Value) -> Option<i64> {
    value
        .as_f64()
        .map(f64::trunc)
        .or_else(|| value.as_i64().map(|n| n as f64))
        .map(f64::trunc)
        .map(|n| n as i64)
}

fn map_ds_code(code: i64) -> u16 {
    match code {
        40003 => 401,
        40002 => 429,
        _ => 502,
    }
}

fn map_ds_biz_code(code: i64) -> u16 {
    match code {
        5 => 403,
        _ => 502,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mute_biz_json_maps_to_credential_restricted() {
        let body = br#"{"code":0,"msg":"","data":{"biz_code":5,"biz_msg":"user is muted","biz_data":{"is_muted":1,"mute_until":1781861651.742}}}"#;
        let err = parse_completion_json_error(body).expect("biz error");
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
    }

    #[test]
    fn non_zero_top_level_code_maps_upstream() {
        let body = br#"{"code":40002,"msg":"rate limited"}"#;
        let err = parse_completion_json_error(body).expect("error");
        match err {
            Error::Upstream { status, .. } => assert_eq!(status, 429),
            other => panic!("expected Upstream, got {other:?}"),
        }
    }

    #[test]
    fn success_json_without_biz_error_returns_none() {
        let body = br#"{"code":0,"data":{"biz_code":0}}"#;
        assert!(parse_completion_json_error(body).is_none());
    }

    #[test]
    fn unknown_biz_code_maps_to_upstream() {
        let body =
            br#"{"code":0,"data":{"biz_code":99,"biz_msg":"maintenance"}}"#;
        let err = parse_completion_json_error(body).expect("biz error");
        match err {
            Error::Upstream { status, message } => {
                assert_eq!(status, 502);
                assert!(message.contains("99"));
            }
            other => panic!("expected Upstream, got {other:?}"),
        }
    }
}

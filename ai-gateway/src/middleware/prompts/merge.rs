use crate::error::{api::ApiError, internal::InternalError};

pub fn merge_prompt_with_request(mut prompt_body: serde_json::Value, request_body: &serde_json::Value) -> Result<serde_json::Value, ApiError> {
    let p_obj = prompt_body.as_object_mut().ok_or(InternalError::Internal)?;
    let r_obj = request_body.as_object().ok_or(InternalError::Internal)?;
    let p_msgs = p_obj.get("messages").and_then(|m| m.as_array()).ok_or(InternalError::Internal)?;
    let r_msgs = r_obj.get("messages").and_then(|m| m.as_array()).ok_or(InternalError::Internal)?;

    let mut merged = p_msgs.clone();
    merged.extend(r_msgs.iter().cloned());
    p_obj.insert("messages".to_string(), serde_json::Value::Array(merged));

    for (k, v) in r_obj { if k != "messages" { p_obj.insert(k.clone(), v.clone()); } }
    Ok(prompt_body)
}

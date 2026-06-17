//! Minimal JSON instances that satisfy a JSON Schema from the request body.

use serde_json::{Map, Value, json};

#[must_use]
pub fn fill_minimal_valid_json(schema: &Value) -> Value {
    if let Some(Value::Array(variants)) =
        schema.get("anyOf").or(schema.get("oneOf"))
    {
        if let Some(first) = variants.first() {
            return fill_minimal_valid_json(first);
        }
    }
    if let Some(Value::Array(parts)) = schema.get("allOf") {
        let mut merged = Map::new();
        for part in parts {
            if let Value::Object(obj) = fill_minimal_valid_json(part) {
                merged.extend(obj);
            }
        }
        return Value::Object(merged);
    }
    if let Some(Value::String(name)) = schema.get("const") {
        return Value::String(name.clone());
    }
    if let Some(Value::Array(items)) = schema.get("enum")
        && let Some(first) = items.first()
    {
        return first.clone();
    }

    match schema.get("type").and_then(Value::as_str) {
        Some("object") => fill_object(schema),
        Some("array") => fill_array(schema),
        Some("string") => Value::String(sample_string(schema)),
        Some("integer") => json!(0),
        Some("number") => json!(0),
        Some("boolean") => json!(true),
        Some("null") => Value::Null,
        _ if schema.get("properties").is_some() => fill_object(schema),
        _ => json!({}),
    }
}

fn fill_object(schema: &Value) -> Value {
    let props = schema.get("properties").and_then(Value::as_object);
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut out = Map::new();
    let Some(props) = props else {
        return Value::Object(out);
    };

    for key in required {
        let Some(child) = props.get(key) else {
            continue;
        };
        out.insert((*key).to_string(), fill_minimal_valid_json(child));
    }
    Value::Object(out)
}

fn fill_array(schema: &Value) -> Value {
    let min = schema
        .get("minItems")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .max(1);
    let item_schema = schema.get("items").unwrap_or(&Value::Null);
    let item = fill_minimal_valid_json(item_schema);
    Value::Array((0..min).map(|_| item.clone()).collect())
}

fn sample_string(schema: &Value) -> String {
    if let Some(Value::Array(items)) = schema.get("enum")
        && let Some(Value::String(s)) = items.first()
    {
        return s.clone();
    }
    "ok".into()
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use web_structured_output::{
        check_structured_response, parse_json_schema_spec,
    };

    use super::*;

    fn assistant(content: &str) -> Value {
        json!({ "choices": [{ "message": { "content": content } }] })
    }

    #[test]
    fn routing_load_fat_schema() {
        let body = json!({
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "strict": true,
                    "schema": {
                        "type": "object",
                        "properties": {
                            "value": { "type": "string" },
                            "details": { "type": "string" }
                        },
                        "required": ["value", "details"],
                        "additionalProperties": false
                    }
                }
            }
        });
        let content = serde_json::to_string(&fill_minimal_valid_json(
            body.pointer("/response_format/json_schema/schema").unwrap(),
        ))
        .unwrap();
        let spec = parse_json_schema_spec(&body).unwrap();
        assert!(
            check_structured_response(&assistant(&content), Some(&spec))
                .is_none()
        );
    }
}

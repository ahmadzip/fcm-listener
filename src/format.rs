use fcm_receiver_rs::messages::{ManualAppData, ManualDataMessageStanza};
use serde_json::{json, Map, Number, Value};

#[derive(Debug, Clone)]
pub struct FormatOptions {
    pub parse_json_strings: bool,
    pub parse_bool_strings: bool,
    pub parse_null_strings: bool,
    pub parse_number_strings: bool,
    pub duplicate_keys_as_array: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            parse_json_strings: true,
            parse_bool_strings: true,
            parse_null_strings: true,
            parse_number_strings: false,
            duplicate_keys_as_array: true,
        }
    }
}

fn parse_app_value(value: &str, options: &FormatOptions) -> Value {
    let trimmed = value.trim();

    if options.parse_json_strings
        && ((trimmed.starts_with('{') && trimmed.ends_with('}'))
            || (trimmed.starts_with('[') && trimmed.ends_with(']')))
    {
        if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
            return parsed;
        }
    }

    if options.parse_bool_strings {
        match trimmed {
            "true" => return Value::Bool(true),
            "false" => return Value::Bool(false),
            _ => {}
        }
    }

    if options.parse_null_strings && trimmed == "null" {
        return Value::Null;
    }

    if options.parse_number_strings {
        if let Ok(v) = trimmed.parse::<i64>() {
            return Value::Number(v.into());
        }
        if let Ok(v) = trimmed.parse::<u64>() {
            return Value::Number(v.into());
        }
        if let Ok(v) = trimmed.parse::<f64>() {
            if let Some(number) = Number::from_f64(v) {
                return Value::Number(number);
            }
        }
    }

    Value::String(value.to_string())
}

fn format_app_data(app_data: &[ManualAppData], options: &FormatOptions) -> Value {
    let mut object = Map::new();

    for item in app_data {
        let value = parse_app_value(&item.value, options);
        match object.get_mut(&item.key) {
            None => {
                object.insert(item.key.clone(), value);
            }
            Some(existing) if options.duplicate_keys_as_array => match existing {
                Value::Array(values) => values.push(value),
                _ => {
                    let previous = std::mem::replace(existing, Value::Null);
                    *existing = Value::Array(vec![previous, value]);
                }
            },
            Some(existing) => {
                *existing = value;
            }
        }
    }

    Value::Object(object)
}

pub fn normalize_raw_message(msg: &ManualDataMessageStanza) -> Value {
    let options = FormatOptions::default();
    json!({
        "id": msg.id,
        "from": msg.from,
        "to": msg.to,
        "category": msg.category,
        "token": msg.token,
        "app_data": format_app_data(&msg.app_data, &options),
        "from_trusted_server": msg.from_trusted_server,
        "persistent_id": msg.persistent_id,
        "stream_id": msg.stream_id,
        "last_stream_id_received": msg.last_stream_id_received,
        "reg_id": msg.reg_id,
        "device_user_id": msg.device_user_id,
        "ttl": msg.ttl,
        "sent": msg.sent,
        "queued": msg.queued,
        "status": msg.status,
        "raw_data_len": msg.raw_data.as_ref().map(|v| v.len()),
        "immediate_ack": msg.immediate_ack,
    })
}

pub fn json_pretty_or_raw(text: &str) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(value) => serde_json::to_string_pretty(&value).unwrap_or_else(|_| text.to_string()),
        Err(_) => text.to_string(),
    }
}

pub fn first_path(value: &Value, paths: &[Vec<&str>]) -> Option<String> {
    'paths: for path in paths {
        let mut current = value;
        for key in path {
            match current.get(*key) {
                Some(next) => current = next,
                None => continue 'paths,
            }
        }
        if let Some(v) = current.as_str() {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

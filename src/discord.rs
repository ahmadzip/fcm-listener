use crate::format::{first_path, json_pretty_or_raw};
use serde_json::{json, Value};
use std::{env, fs, process::Command};

fn find_json_object_after(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)? + marker.len();
    let bytes = text.as_bytes();
    let mut i = start;
    while i < bytes.len() && bytes[i] != b'{' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }

    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (idx, ch) in text[i..].char_indices() {
        if esc {
            esc = false;
            continue;
        }
        if ch == '\\' && in_str {
            esc = true;
            continue;
        }
        if ch == '"' {
            in_str = !in_str;
            continue;
        }
        if in_str {
            continue;
        }
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                return Some(text[i..i + idx + ch.len_utf8()].to_string());
            }
        }
    }
    None
}

fn summary_from_value(title: &str, value: &Value) -> String {
    let from = first_path(
        value,
        &[
            vec!["category"],
            vec!["packageName"],
            vec!["package"],
            vec!["pkg"],
            vec!["app_data", "data", "packageName"],
            vec!["app_data", "data", "package"],
        ],
    )
    .unwrap_or_else(|| "com.gojek.gopaymerchant".to_string());

    let notif_title = first_path(
        value,
        &[
            vec!["title"],
            vec!["notification_title"],
            vec!["app_data", "data", "title"],
            vec!["app_data", "title"],
            vec!["app_data", "data", "notification_title"],
        ],
    )
    .unwrap_or_else(|| "-".to_string());

    let text = first_path(
        value,
        &[
            vec!["text"],
            vec!["body"],
            vec!["message"],
            vec!["notification_text"],
            vec!["app_data", "data", "text"],
            vec!["app_data", "data", "body"],
            vec!["app_data", "text"],
            vec!["app_data", "body"],
            vec!["app_data", "data", "message"],
            vec!["app_data", "data", "notification_text"],
        ],
    )
    .unwrap_or_else(|| "-".to_string());

    format!("**{title}**\nfrom: {from}\ntitle: {notif_title}\ntext: {text}")
}

pub fn message_summary(title: &str, body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        return summary_from_value(title, &value);
    }

    if let Some(data_json) = find_json_object_after(body, "value: \"") {
        if let Ok(value) = serde_json::from_str::<Value>(&data_json) {
            return summary_from_value(title, &value);
        }
    }

    let mut lines = vec![format!("**{title}**")];
    for needle in ["title", "text", "category", "persistent_id"] {
        if let Some(pos) = body.find(needle) {
            let snippet: String = body[pos..].chars().take(160).collect();
            lines.push(snippet.replace('\n', " "));
        }
    }
    if lines.len() == 1 {
        lines.push("Full payload attached below.".to_string());
    }
    lines.join("\n")
}

pub fn send_file(webhook_url: &str, title: &str, body: &str) {
    if webhook_url.is_empty() {
        return;
    }

    let pretty = json_pretty_or_raw(body);
    let payload = json!({ "content": message_summary(title, &pretty) }).to_string();
    let filename = format!("{}.json", title.replace(' ', "_").to_lowercase());
    let path = env::temp_dir().join(&filename);

    if let Err(err) = fs::write(&path, &pretty) {
        eprintln!("discord_file_write_failed err={err}");
        return;
    }

    let file_arg = format!("file=@{};filename={filename}", path.display());
    let form_arg = format!("payload_json={payload}");

    match Command::new("curl")
        .args([
            "-sS",
            "--connect-timeout",
            "5",
            "--max-time",
            "15",
            "-X",
            "POST",
            "-F",
            &form_arg,
            "-F",
            &file_arg,
            webhook_url,
        ])
        .output()
    {
        Ok(out) if out.status.success() => println!("discord_file_sent title={title}"),
        Ok(out) => eprintln!(
            "discord_file_failed status={} stderr={} stdout={}",
            out.status,
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        ),
        Err(err) => eprintln!("discord_file_failed err={err}"),
    }

    let _ = fs::remove_file(path);
}

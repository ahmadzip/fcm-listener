use crate::format::first_path;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
struct PaymentPayload {
    #[serde(rename = "packageName")]
    package_name: String,
    title: String,
    text: String,
    #[serde(rename = "postedAt")]
    posted_at: u64,
    #[serde(rename = "notificationKey")]
    notification_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PaymentWebhookBody {
    payload: PaymentPayload,
}

fn body_from_value(value: &Value, fallback_text: &str) -> PaymentWebhookBody {
    PaymentWebhookBody {
        payload: PaymentPayload {
            package_name: first_path(
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
            .unwrap_or_else(|| "com.gojek.gopaymerchant".to_string()),
            title: first_path(
                value,
                &[
                    vec!["app_data", "data", "title"],
                    vec!["app_data", "title"],
                    vec!["title"],
                    vec!["notification_title"],
                ],
            )
            .unwrap_or_else(|| "GoPay notification".to_string()),
            text: first_path(
                value,
                &[
                    vec!["app_data", "data", "text"],
                    vec!["app_data", "data", "body"],
                    vec!["app_data", "body"],
                    vec!["app_data", "text"],
                    vec!["text"],
                    vec!["body"],
                    vec!["message"],
                    vec!["notification_text"],
                ],
            )
            .unwrap_or_else(|| fallback_text.to_string()),
            posted_at: first_path(
                value,
                &[
                    vec!["app_data", "data", "postedAt"],
                    vec!["app_data", "data", "posted_at"],
                    vec!["postedAt"],
                    vec!["posted_at"],
                ],
            )
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0),
            notification_key: first_path(
                value,
                &[
                    vec!["persistent_id"],
                    vec!["notificationKey"],
                    vec!["notification_key"],
                    vec!["id"],
                    vec!["app_data", "data", "notificationKey"],
                    vec!["app_data", "data", "notification_key"],
                ],
            )
            .unwrap_or_default(),
        },
    }
}

fn post_json(url: &str, token: &str, body: &str) -> bool {
    if url.is_empty() {
        return false;
    }

    let mut cmd = Command::new("curl");
    cmd.args([
        "-sS",
        "--connect-timeout",
        "5",
        "--max-time",
        "15",
        "-X",
        "POST",
        "-H",
        "Content-Type: application/json",
    ]);
    if !token.is_empty() {
        let mut header = String::from("Authorization: Bearer ");
        header.push_str(token);
        cmd.args(["-H", &header]);
    }
    cmd.args(["--data-raw", body, url]);

    match cmd.output() {
        Ok(out) if out.status.success() => true,
        Ok(out) => {
            eprintln!(
                "webhook_failed status={} stderr={} stdout={}",
                out.status,
                String::from_utf8_lossy(&out.stderr),
                String::from_utf8_lossy(&out.stdout)
            );
            false
        }
        Err(err) => {
            eprintln!("webhook_failed err={err}");
            false
        }
    }
}

pub fn send_value(url: &str, token: &str, value: &Value, fallback_text: &str) {
    if url.is_empty() {
        return;
    }

    let body = match serde_json::to_string(&body_from_value(value, fallback_text)) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("webhook_payload_failed err={err}");
            return;
        }
    };

    if post_json(url, token, &body) {
        println!("webhook_sent");
    }
}

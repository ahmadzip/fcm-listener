mod discord;
mod format;
mod webhook;
mod state;

use fcm_receiver_rs::client::{AndroidApp, FcmClient};
use format::normalize_raw_message;
use serde_json::{json, Value};
use state::{
    is_new_since_start, load_seen, load_state, now_ms, remember_seen, save_seen, save_state,
    FcmState, STATE_FILE,
};
use std::{
    env,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let app_id = env::var("APP_ID")?;
    let api_key = env::var("API_KEY")?;
    let gcm_sender_id = env::var("GCM_SENDER_ID")?;
    let project_id = env::var("PROJECT_ID")?;
    let package = env::var("FCM_ANDROID_PACKAGE")?;
    let cert = env::var("FCM_ANDROID_CERT").unwrap_or_default();
    let env_value = |name: &str| env::var(name).unwrap_or_default().trim().to_string();
    let required_env = |name: &str| {
        let value = env_value(name);
        if value.is_empty() {
            eprintln!("missing {name}");
            std::process::exit(2);
        }
        value
    };

    let discord_webhook_url = env_value("DISCORD_WEBHOOK_URL");
    let webhook_url = {
        let value = env_value("WEBHOOK_URL");
        if value.is_empty() {
            env_value("QRIS_WEBHOOK_URL")
        } else {
            value
        }
    };
    if webhook_url.is_empty() {
        eprintln!("missing WEBHOOK_URL");
        std::process::exit(2);
    }
    let webhook_token = required_env("WEBHOOK_TOKEN");

    if cert.is_empty() {
        eprintln!("missing FCM_ANDROID_CERT");
        eprintln!("contoh: set FCM_ANDROID_CERT=<sha1 cert fingerprint tanpa colon>");
        eprintln!("package default: {package}");
        std::process::exit(2);
    }

    let mut client = FcmClient::new(api_key, app_id, project_id)?;
    let process_started_ms = now_ms();
    client.android_app = Some(AndroidApp {
        gcm_sender_id,
        android_package: package,
        android_package_cert: cert,
    });

    if let Some(state) = load_state() {
        client.android_id = state.android_id;
        client.security_token = state.security_token;
        client.fcm_token = Some(state.fcm_token.clone());
        client.gcm_token = if state.gcm_token.is_empty() {
            None
        } else {
            Some(state.gcm_token.clone())
        };
        client.load_keys(&state.private_key, &state.auth_secret)?;

        println!(
            "state_loaded android_id={} fcm_token_len={}",
            state.android_id,
            state.fcm_token.len()
        );
    } else {
        let (private_key, auth_secret) = client.create_new_keys()?;
        let (fcm_token, gcm_token, android_id, security_token) = client.register()?;

        let state = FcmState {
            android_id,
            security_token,
            fcm_token,
            gcm_token,
            private_key,
            auth_secret,
        };
        save_state(&state)?;

        println!(
            "register_ok state_saved={STATE_FILE} android_id={} fcm_token_len={}",
            state.android_id,
            state.fcm_token.len()
        );
    }

    let heartbeat_interval_secs = env::var("FCM_HEARTBEAT_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(120);
    let heartbeat_ack_timeout_secs = env::var("FCM_HEARTBEAT_ACK_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);

    client.heartbeat_interval = Duration::from_secs(heartbeat_interval_secs);
    client.heartbeat_ack_timeout = Duration::from_secs(heartbeat_ack_timeout_secs);

    println!(
        "[FCM] heartbeat interval={}s ack_timeout={}s",
        heartbeat_interval_secs, heartbeat_ack_timeout_secs
    );

    let mut seen_items = load_seen();
    save_seen(&seen_items);
    println!("seen_loaded count={}", seen_items.len());

    let seen = Arc::new(Mutex::new(std::mem::take(&mut seen_items)));
    let raw_webhook = Arc::new(discord_webhook_url.clone());
    let raw_webhook_url = Arc::new(webhook_url.clone());
    let raw_webhook_token = Arc::new(webhook_token.clone());
    let raw_seen = Arc::clone(&seen);

    client.on_raw_message = Some(Arc::new(move |msg| {
        if !is_new_since_start(process_started_ms, msg.sent) {
            println!(
                "old_skipped sent={:?} persistent_id={}",
                msg.sent,
                msg.persistent_id.as_deref().unwrap_or("")
            );
            return;
        }

        if let Some(pid) = msg.persistent_id.as_deref() {
            if !remember_seen(&raw_seen, pid) {
                println!("duplicate_skipped persistent_id={pid}");
                return;
            }
        }

        let payload = normalize_raw_message(&msg);
        let json = serde_json::to_string(&payload).unwrap_or_else(|_| json!({ "error": "format_failed" }).to_string());
        let pretty = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| json.clone());
        let summary = discord::message_summary("FCM", &json).replace('\n', " | ");
        println!("notification {summary}");
        discord::send_file(&raw_webhook, "FCM raw message", &json);
        webhook::send_value(&raw_webhook_url, &raw_webhook_token, &payload, &pretty);
    }));

    let data_discord = Arc::new(discord_webhook_url);
    let data_webhook_url = Arc::new(webhook_url);
    let data_webhook_token = Arc::new(webhook_token);

    client.on_data_message = Some(Arc::new(move |data| {
        let text = String::from_utf8_lossy(&data).to_string();
        let payload = serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({ "text": text }));
        let json = serde_json::to_string(&payload).unwrap_or_else(|_| json!({ "error": "format_failed" }).to_string());
        let pretty = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| json.clone());
        let summary = discord::message_summary("FCM", &json).replace('\n', " | ");
        println!("notification {summary}");
        discord::send_file(&data_discord, "FCM data message", &json);
        webhook::send_value(&data_webhook_url, &data_webhook_token, &payload, &pretty);
    }));

    run_listener_forever(&mut client);
    Ok(())
}

fn run_listener_forever(client: &mut FcmClient) {
    const BACKOFF_SECS: [u64; 6] = [2, 4, 8, 16, 30, 60];
    const HEALTHY_SESSION_SECS: u64 = 180;

    let mut retry = 0usize;

    loop {
        println!("[FCM] starting_listener");
        let started_at = Instant::now();
        let result = client.start_listening();
        let session_age = started_at.elapsed();

        // Ensure a failed TLS/MCS socket is closed before the backoff sleep.
        // This does not re-register FCM and does not alter the saved credentials.
        client.close();

        if session_age >= Duration::from_secs(HEALTHY_SESSION_SECS) {
            retry = 0;
        }

        match result {
            Ok(()) => eprintln!("[FCM] listener_stopped"),
            Err(err) => eprintln!("[FCM] listener_error: {err}"),
        }

        let delay = BACKOFF_SECS[retry.min(BACKOFF_SECS.len() - 1)];
        eprintln!("[FCM] reconnecting in {delay}s");
        thread::sleep(Duration::from_secs(delay));

        retry = (retry + 1).min(BACKOFF_SECS.len() - 1);
    }
}

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub const STATE_FILE: &str = "fcm_state.json";
pub const SEEN_FILE: &str = "seen_persistent_ids.json";
const SEEN_MAX: usize = 1000;
const SEEN_TTL_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Serialize, Deserialize)]
pub struct FcmState {
    pub android_id: u64,
    pub security_token: u64,
    pub fcm_token: String,
    pub gcm_token: String,
    pub private_key: String,
    pub auth_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeenPersistentId {
    pub id: String,
    pub ts: u64,
}

pub fn load_state() -> Option<FcmState> {
    let text = fs::read_to_string(Path::new(STATE_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_state(state: &FcmState) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(STATE_FILE, serde_json::to_string_pretty(state)?)?;
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn prune_seen(mut items: Vec<SeenPersistentId>) -> Vec<SeenPersistentId> {
    let cutoff = now_ms().saturating_sub(SEEN_TTL_MS);
    items.retain(|item| item.ts >= cutoff && !item.id.is_empty());
    items.sort_by_key(|item| item.ts);
    if items.len() > SEEN_MAX {
        items = items[items.len() - SEEN_MAX..].to_vec();
    }
    items
}

pub fn load_seen() -> Vec<SeenPersistentId> {
    let text = match fs::read_to_string(Path::new(SEEN_FILE)) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str::<Vec<SeenPersistentId>>(&text)
        .map(prune_seen)
        .unwrap_or_default()
}

pub fn save_seen(items: &[SeenPersistentId]) {
    if let Ok(text) = serde_json::to_string_pretty(items) {
        let _ = fs::write(SEEN_FILE, text);
    }
}

pub fn remember_seen(seen: &Arc<Mutex<Vec<SeenPersistentId>>>, id: &str) -> bool {
    let mut items = seen.lock().unwrap();
    *items = prune_seen(std::mem::take(&mut *items));
    if items.iter().any(|item| item.id == id) {
        return false;
    }

    items.push(SeenPersistentId {
        id: id.to_string(),
        ts: now_ms(),
    });
    *items = prune_seen(std::mem::take(&mut *items));
    save_seen(&items);
    true
}

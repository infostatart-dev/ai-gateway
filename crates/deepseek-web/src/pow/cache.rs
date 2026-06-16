use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::api::pow_challenge::PowChallenge;

const TTL: Duration = Duration::from_secs(45);
const MAX_ENTRIES: usize = 64;

#[derive(Debug, Clone)]
struct CacheEntry {
    challenge: PowChallenge,
    pow_response: String,
    expires: Instant,
}

#[derive(Debug, Default)]
pub struct PowCache {
    inner: Mutex<HashMap<String, CacheEntry>>,
    hits: Mutex<u32>,
}

impl PowCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cache_hits(&self) -> u32 {
        *self.hits.lock().expect("pow cache hits lock")
    }

    pub fn get(
        &self,
        access_token: &str,
        session_id: &str,
        challenge: &PowChallenge,
    ) -> Option<String> {
        let key = cache_key(access_token, session_id);
        let mut map = self.inner.lock().expect("pow cache lock");
        let entry = map.get(&key)?;
        if entry.expires <= Instant::now() || entry.challenge != *challenge {
            map.remove(&key);
            return None;
        }
        *self.hits.lock().expect("pow cache hits lock") += 1;
        Some(entry.pow_response.clone())
    }

    pub fn store(
        &self,
        access_token: &str,
        session_id: &str,
        challenge: PowChallenge,
        pow_response: String,
    ) {
        let key = cache_key(access_token, session_id);
        let mut map = self.inner.lock().expect("pow cache lock");
        if map.len() >= MAX_ENTRIES {
            map.clear();
        }
        map.insert(
            key,
            CacheEntry {
                challenge,
                pow_response,
                expires: Instant::now() + TTL,
            },
        );
    }

    pub fn invalidate(&self, access_token: &str, session_id: &str) {
        let key = cache_key(access_token, session_id);
        self.inner.lock().expect("pow cache lock").remove(&key);
    }
}

fn cache_key(access_token: &str, session_id: &str) -> String {
    let prefix = &access_token[..access_token.len().min(12)];
    format!("{prefix}:{session_id}")
}

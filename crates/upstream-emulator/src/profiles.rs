use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForcedProfile {
    AuthError,
    QuotaExhausted,
    Overload,
    NotFound,
    HighDemand,
}

#[derive(Debug, Default, Clone)]
pub struct AdminProfiles {
    forced: Arc<Mutex<Vec<(String, ForcedProfile)>>>,
}

impl AdminProfiles {
    pub fn force(&self, scope: &str, profile: ForcedProfile) {
        self.forced
            .lock()
            .expect("admin profiles")
            .push((scope.to_string(), profile));
    }

    pub fn forced_profile(&self, scope: &str) -> Option<ForcedProfile> {
        self.forced
            .lock()
            .expect("admin profiles")
            .iter()
            .rev()
            .find(|(key, _)| key == scope)
            .map(|(_, profile)| *profile)
    }

    pub fn reset(&self) {
        self.forced.lock().expect("admin profiles").clear();
    }
}

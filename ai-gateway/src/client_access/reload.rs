use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    time::{Duration, SystemTime},
};

use meltdown::Token;

use crate::{app_state::AppState, error::runtime::RuntimeError};

#[derive(Debug, Clone)]
pub struct ClientAccessReloader {
    app_state: AppState,
    path: PathBuf,
    interval: Duration,
}

impl ClientAccessReloader {
    #[must_use]
    pub fn new(app_state: AppState, path: PathBuf, interval: Duration) -> Self {
        Self {
            app_state,
            path,
            interval,
        }
    }
}

impl meltdown::Service for ClientAccessReloader {
    type Future =
        Pin<Box<dyn Future<Output = Result<(), RuntimeError>> + Send>>;

    fn run(self, token: Token) -> Self::Future {
        Box::pin(async move {
            run_reloader(self.app_state, self.path, self.interval, token).await;
            Ok(())
        })
    }
}

async fn run_reloader(
    app_state: AppState,
    path: PathBuf,
    interval: Duration,
    token: Token,
) {
    let mut last_seen = file_stamp(&path);
    loop {
        let shutdown = token.clone();
        tokio::select! {
            biased;
            () = shutdown => {
                tracing::info!("client access reloader shutting down");
                return;
            }
            () = tokio::time::sleep(interval) => {
                reload_if_changed(&app_state, &path, &mut last_seen);
            }
        }
    }
}

fn reload_if_changed(
    app_state: &AppState,
    path: &std::path::Path,
    last_seen: &mut Option<(SystemTime, u64)>,
) -> bool {
    let current = file_stamp(path);
    if current == *last_seen {
        return false;
    }
    *last_seen = current;
    match crate::client_access::loader::load_snapshot_from_file(path) {
        Ok(snapshot) => {
            let keys = snapshot.len();
            if app_state.set_client_access_snapshot(snapshot) {
                app_state
                    .0
                    .metrics
                    .client_access
                    .reload_successes
                    .add(1, &[]);
                tracing::info!(
                    path = %path.display(),
                    keys,
                    "client access snapshot reloaded",
                );
            } else {
                tracing::warn!(
                    path = %path.display(),
                    "client access reload skipped because snapshot holder is disabled",
                );
            }
        }
        Err(error) => {
            app_state
                .0
                .metrics
                .client_access
                .reload_failures
                .add(1, &[]);
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "client access reload failed; keeping last valid snapshot",
            );
        }
    }
    true
}

fn file_stamp(path: &std::path::Path) -> Option<(SystemTime, u64)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

#[cfg(all(test, feature = "testing"))]
mod tests {
    use std::path::PathBuf;

    use uuid::Uuid;

    use super::*;
    use crate::{
        app::App,
        client_access::ClientAccessKeyHash,
        config::{
            Config,
            client_access::{ClientAccessConfig, ClientAccessQuotaStoreConfig},
        },
        tests::TestDefault,
    };

    fn temp_registry_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "ai-gateway-client-access-reload-{}-{}.yaml",
            std::process::id(),
            Uuid::new_v4()
        ))
    }

    fn registry_yaml(token: &str, key_id: &str) -> String {
        let hash = ClientAccessKeyHash::from_bearer_token(token);
        format!(
            r#"
version: 1
subjects:
  acme:
    org-id: "00000000-0000-0000-0000-000000000001"
    user-id: "00000000-0000-0000-0000-000000000002"
plans:
  starter:
    limits:
      requests:
        per-minute: 10
      tokens:
        per-minute: 1000
keys:
  {key_id}:
    hash: "{hash}"
    subject: acme
    status: active
    plan: starter
    scopes:
      - unified-api
"#
        )
    }

    fn empty_keys_yaml() -> &'static str {
        r#"
version: 1
plans:
  starter:
    limits:
      requests:
        per-minute: 10
      tokens:
        per-minute: 1000
keys: {}
"#
    }

    async fn app_state_for_file(path: PathBuf) -> AppState {
        let mut config = Config::test_default();
        config.client_access = ClientAccessConfig {
            enabled: true,
            file: Some(path),
            reload_interval: Duration::from_millis(10),
            max_body_bytes: 1024,
            quota_store: ClientAccessQuotaStoreConfig::Memory,
        };
        App::new(config).await.unwrap().state
    }

    #[tokio::test]
    async fn client_access_reload_valid_file_revokes_removed_key() {
        let path = temp_registry_path();
        std::fs::write(&path, registry_yaml("old-token", "old-key")).unwrap();
        let app_state = app_state_for_file(path.clone()).await;
        let mut stamp = file_stamp(&path);

        std::fs::write(&path, empty_keys_yaml()).unwrap();
        assert!(reload_if_changed(&app_state, &path, &mut stamp));

        assert!(
            app_state
                .client_access_snapshot()
                .unwrap()
                .lookup_bearer_token("old-token")
                .is_none()
        );
    }

    #[tokio::test]
    async fn client_access_reload_invalid_yaml_keeps_last_good_snapshot() {
        let path = temp_registry_path();
        std::fs::write(&path, registry_yaml("stable-token", "stable-key"))
            .unwrap();
        let app_state = app_state_for_file(path.clone()).await;
        let mut stamp = file_stamp(&path);

        std::fs::write(&path, "version: [").unwrap();
        assert!(reload_if_changed(&app_state, &path, &mut stamp));

        assert!(
            app_state
                .client_access_snapshot()
                .unwrap()
                .lookup_bearer_token("stable-token")
                .is_some()
        );
    }

    #[tokio::test]
    async fn client_access_reload_deleted_file_keeps_last_good_snapshot() {
        let path = temp_registry_path();
        std::fs::write(&path, registry_yaml("stable-token", "stable-key"))
            .unwrap();
        let app_state = app_state_for_file(path.clone()).await;
        let mut stamp = file_stamp(&path);

        std::fs::remove_file(&path).unwrap();
        assert!(reload_if_changed(&app_state, &path, &mut stamp));

        assert!(
            app_state
                .client_access_snapshot()
                .unwrap()
                .lookup_bearer_token("stable-token")
                .is_some()
        );
    }
}

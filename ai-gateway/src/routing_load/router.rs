use crate::{
    config::secrets_file::SECRETS_FILE_ENV,
    router::budget_aware::clear_test_call_responses,
};

pub struct RoutingLoadHarness {
    pub secrets_path: std::path::PathBuf,
}

impl RoutingLoadHarness {
    pub fn gemini_prod_like(free_slots: u8) -> Self {
        Self::gemini_slots(free_slots, true)
    }

    pub fn gemini_free_only(free_slots: u8) -> Self {
        Self::gemini_slots(free_slots, false)
    }

    fn gemini_slots(free_slots: u8, include_paid: bool) -> Self {
        let dir = std::env::temp_dir()
            .join(format!("ai-gw-routing-load-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let mut yaml = String::from("credentials:\n");
        for index in 1..=free_slots {
            let id = if index == 1 {
                "gemini-free".to_string()
            } else {
                format!("gemini-free-{index}")
            };
            yaml.push_str("  ");
            yaml.push_str(&id);
            yaml.push_str(":\n    api-key: free-");
            yaml.push_str(&index.to_string());
            yaml.push_str("-key\n");
        }
        if include_paid {
            yaml.push_str("  gemini-default:\n    api-key: paid-key\n");
        }
        let path = dir.join("secrets.yaml");
        std::fs::write(&path, yaml).expect("secrets");
        unsafe {
            std::env::set_var(SECRETS_FILE_ENV, &path);
        }
        Self { secrets_path: dir }
    }

    pub fn apply_credentials(&self, config: &mut crate::config::Config) {
        let path = self.secrets_path.join("secrets.yaml");
        let mut secrets = crate::config::secrets_file::SecretsFile::load(&path)
            .expect("load routing-load secrets");
        secrets.integrations.aws =
            Some(crate::config::secrets_file::AwsSecret {
                access_key: "test-access-key".into(),
                secret_key: "test-secret-key".into(),
                region: "us-east-1".into(),
            });
        config.credentials =
            crate::config::credentials::CredentialRegistry::build(
                &config.providers,
                &mut secrets,
            );
        crate::config::secrets_file::SecretsFile::install(secrets);
    }
}

impl Drop for RoutingLoadHarness {
    fn drop(&mut self) {
        clear_test_call_responses();
        let _ = std::fs::remove_dir_all(&self.secrets_path);
    }
}

pub fn prepare_harness_test() {
    clear_test_call_responses();
}

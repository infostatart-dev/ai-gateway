pub const CHATGPT_BASE: &str = "https://chatgpt.com";
pub const SESSION_URL: &str = "https://chatgpt.com/api/auth/session";
pub const SENTINEL_PREPARE_URL: &str =
    "https://chatgpt.com/backend-api/sentinel/chat-requirements/prepare";
pub const SENTINEL_CR_URL: &str =
    "https://chatgpt.com/backend-api/sentinel/chat-requirements";
pub const CONV_URL: &str = "https://chatgpt.com/backend-api/f/conversation";
pub const USER_LAST_USED_MODEL_CONFIG_URL: &str =
    "https://chatgpt.com/backend-api/settings/user_last_used_model_config";

pub const CHATGPT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X \
                                      10.15; rv:152.0) Gecko/20100101 \
                                      Firefox/152.0";

pub const OAI_CLIENT_VERSION: &str =
    "prod-81e0c5cdf6140e8c5db714d613337f4aeab94029";
pub const OAI_CLIENT_BUILD_NUMBER: &str = "6128297";

pub const TOKEN_TTL_MS: u64 = 5 * 60 * 1000;
pub const DPL_TTL_MS: u64 = 60 * 60 * 1000;
pub const SESSION_ENV: &str = "CHATGPT_BROWSER_CLI";

pub use web_structured_output::{JSON_RETRY_SUFFIX, SCHEMA_RETRY_SUFFIX};

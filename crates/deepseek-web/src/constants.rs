pub const SESSION_ENV: &str = "DEEPSEEK_BROWSER_CLI";
pub const USER_TOKEN_STORAGE_KEY: &str = "userToken";

pub const DEEPSEEK_WEB_BASE: &str = "https://chat.deepseek.com";
pub const DEEPSEEK_API_BASE: &str = "https://chat.deepseek.com/api";

pub const USERS_CURRENT_URL: &str =
    "https://chat.deepseek.com/api/v0/users/current";
pub const SESSION_CREATE_URL: &str =
    "https://chat.deepseek.com/api/v0/chat_session/create";
pub const SESSION_DELETE_URL: &str =
    "https://chat.deepseek.com/api/v0/chat_session/delete";
pub const POW_CHALLENGE_URL: &str =
    "https://chat.deepseek.com/api/v0/chat/create_pow_challenge";
pub const COMPLETION_URL: &str =
    "https://chat.deepseek.com/api/v0/chat/completion";

pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                              AppleWebKit/537.36 (KHTML, like Gecko) \
                              Chrome/134.0.0.0 Safari/537.36";
pub const APP_VERSION: &str = "20241129.1";
pub const CLIENT_LOCALE: &str = "en-US";
pub const CLIENT_PLATFORM: &str = "web";
pub const CLIENT_VERSION: &str = "1.8.0";

pub const TOKEN_TTL_SECS: u64 = 3600;
pub const POW_TARGET_PATH: &str = "/api/v0/chat/completion";

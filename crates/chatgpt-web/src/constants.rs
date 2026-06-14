pub const CHATGPT_BASE: &str = "https://chatgpt.com";
pub const SESSION_URL: &str = "https://chatgpt.com/api/auth/session";
pub const SENTINEL_PREPARE_URL: &str =
    "https://chatgpt.com/backend-api/sentinel/chat-requirements/prepare";
pub const SENTINEL_CR_URL: &str =
    "https://chatgpt.com/backend-api/sentinel/chat-requirements";
pub const CONV_URL: &str = "https://chatgpt.com/backend-api/f/conversation";

pub const CHATGPT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:148.0) \
    Gecko/20100101 Firefox/148.0";

pub const OAI_CLIENT_VERSION: &str = "prod-81e0c5cdf6140e8c5db714d613337f4aeab94029";
pub const OAI_CLIENT_BUILD_NUMBER: &str = "6128297";

pub const TOKEN_TTL_MS: u64 = 5 * 60 * 1000;
pub const DPL_TTL_MS: u64 = 60 * 60 * 1000;
pub const SESSION_ENV: &str = "CHATGPT_BROWSER_CLI";

pub const JSON_RETRY_SUFFIX: &str = "\n\nCRITICAL: Your previous response was not valid JSON. \
    Reply with ONLY a JSON object matching the schema. No markdown fences, no prose.";

pub const SCHEMA_RETRY_SUFFIX: &str = "\n\nCRITICAL: Your previous JSON did not match the required \
    schema. Focus carefully: output ONLY a corrected JSON object that satisfies every required field \
    and type in the schema. Preserve all factual content — do not drop information. No prose, no \
    markdown fences.";

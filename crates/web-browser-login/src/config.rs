/// Target site for headed browser login (OmniRoute uses manual import; this is
/// CLI convenience).
#[derive(Debug, Clone, Copy)]
pub struct BrowserLoginTarget {
    pub login_url: &'static str,
    pub home_url: &'static str,
    pub timeout_secs: u64,
}

impl BrowserLoginTarget {
    pub const fn new(login_url: &'static str, home_url: &'static str) -> Self {
        Self {
            login_url,
            home_url,
            timeout_secs: 300,
        }
    }

    pub const fn with_timeout(self, timeout_secs: u64) -> Self {
        Self {
            login_url: self.login_url,
            home_url: self.home_url,
            timeout_secs,
        }
    }
}

pub fn chatgpt_domain(domain: &str) -> bool {
    domain.contains("chatgpt.com") || domain.contains("openai.com")
}

pub fn perplexity_domain(domain: &str) -> bool {
    domain.contains("perplexity.ai")
}

pub fn perplexity_left_login(url: &str) -> bool {
    !url.contains("/auth/") && url.contains("perplexity.ai")
}

pub fn chatgpt_left_login(url: &str) -> bool {
    !url.contains("/auth/login") && url.contains("chatgpt.com")
}

pub fn deepseek_domain(url: &str) -> bool {
    url.contains("deepseek.com")
}

/// Poll storage only after the user left the sign-in flow.
pub fn deepseek_ready_url(url: &str) -> bool {
    deepseek_domain(url) && !url.contains("/sign_in")
}

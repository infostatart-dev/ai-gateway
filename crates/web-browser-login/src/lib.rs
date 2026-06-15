//! Shared headed-browser login for web-session providers.
//!
//! OmniRoute stores Perplexity/ChatGPT cookies via credentials config (manual
//! DevTools import). This crate implements the same poll loop for `*-web login`
//! CLI commands.

mod browser;
mod config;
mod options;
mod poll;

pub use browser::{
    default_user_data_dir, system_chrome_executable, wipe_user_data_dir,
};
pub use config::{
    BrowserLoginTarget, chatgpt_domain, chatgpt_left_login, perplexity_domain,
    perplexity_left_login,
};
pub use options::PollOptions;
pub use poll::{poll_session_cookie, poll_session_cookie_with_options};

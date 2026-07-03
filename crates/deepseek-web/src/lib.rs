//! DeepSeek web (chat.deepseek.com) session client for browser-authenticated
//! chat completions.

pub mod api;
pub mod biz_error;
pub mod completion;
pub mod constants;
pub mod cookie;
pub mod errors;
pub mod executor;
pub mod headers;
pub mod pow;
pub mod session;
pub mod sse;
pub mod tls;

#[cfg(feature = "login")]
pub mod login;

pub use constants::{COMPLETION_URL, DEEPSEEK_WEB_BASE, SESSION_ENV};
pub use errors::Error;
pub use executor::{
    ExecuteRequest, ExecuteResult, ExecuteStats, Executor, TurnHook,
};
pub use session::{
    file::{
        BrowserSession, SessionFile, load_session, save_session,
        session_path_from_env,
    },
    token::normalize_user_token,
};

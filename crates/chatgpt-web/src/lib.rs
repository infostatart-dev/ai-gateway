//! ChatGPT web (chatgpt.com) session client for browser-authenticated chat completions.

pub mod constants;
pub mod conversation;
pub mod errors;
pub mod executor;
pub mod headers;
pub mod models;
pub mod schema;
pub mod sentinel;
pub mod session;
pub mod tls;

#[cfg(feature = "login")]
pub mod login;

pub use constants::CONV_URL;
pub use errors::Error;
pub use executor::{ExecuteRequest, ExecuteResult, Executor};
pub use session::file::{load_session, save_session, session_path_from_env, SessionFile};

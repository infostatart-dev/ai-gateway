//! Perplexity web (perplexity.ai) session client.

pub mod constants;
pub mod errors;
pub mod messages;
pub mod probe;
pub mod session;
pub mod tls;

#[cfg(feature = "login")]
pub mod login;

pub use constants::SESSION_ENV;
pub use errors::Error;
#[cfg(feature = "login")]
pub use login::{run_login, save_session_from_cookie};
pub use messages::{
    build_turn_query, plan_perplexity_turns, prepare_turn_plan_from_messages,
};
pub use probe::{ProbeResult, probe_query};
pub use session::file::{
    SessionFile, load_session, save_session, session_path_from_env,
};

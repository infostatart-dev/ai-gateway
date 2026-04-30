pub mod request;
pub mod response;
pub mod stream;
pub mod error;
pub mod message;
pub mod tool;

pub use request::AnthropicConverter;
pub const OPENAI_CHAT_COMPLETION_OBJECT: &str = "chat.completion";

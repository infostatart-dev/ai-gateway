mod body;
mod model;
mod prompt;

pub use body::{
    CompletionRequest, build_completion_request, completion_headers,
    completion_json,
};
pub use model::{ModelOptions, resolve_model_options};
pub use prompt::messages_to_prompt;

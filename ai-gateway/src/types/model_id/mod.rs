pub mod base;
pub mod bedrock;
pub mod id;
pub mod name;
pub mod ollama;
pub mod parsing;
pub mod version;

pub use base::ModelIdWithVersion;
pub use bedrock::BedrockModelId;
pub use id::{ModelId, ModelIdWithoutVersion};
pub use name::ModelName;
pub use ollama::OllamaModelId;
pub use version::Version;

#[cfg(test)]
mod tests;

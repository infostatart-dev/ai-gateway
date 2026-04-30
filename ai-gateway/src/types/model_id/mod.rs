pub mod version;
pub mod name;
pub mod base;
pub mod ollama;
pub mod bedrock;
pub mod parsing;
pub mod id;

pub use version::Version;
pub use name::ModelName;
pub use base::ModelIdWithVersion;
pub use ollama::OllamaModelId;
pub use bedrock::BedrockModelId;
pub use id::{ModelId, ModelIdWithoutVersion};

#[cfg(test)]
mod tests;

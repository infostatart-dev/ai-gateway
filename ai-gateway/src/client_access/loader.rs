use std::{path::Path, sync::Arc};

use crate::{
    client_access::{ClientAccessSnapshot, ClientAccessSnapshotError},
    config::client_access::ClientAccessRegistryFile,
};

pub fn load_snapshot_from_file(
    path: &Path,
) -> Result<Arc<ClientAccessSnapshot>, ClientAccessLoadError> {
    let raw = std::fs::read_to_string(path).map_err(|source| {
        ClientAccessLoadError::Read {
            path: path.display().to_string(),
            source,
        }
    })?;
    let registry: ClientAccessRegistryFile = serde_yml::from_str(&raw)
        .map_err(|source| ClientAccessLoadError::Parse {
            path: path.display().to_string(),
            source,
        })?;
    let snapshot =
        ClientAccessSnapshot::from_registry(registry).map_err(|source| {
            ClientAccessLoadError::Validate {
                path: path.display().to_string(),
                source,
            }
        })?;
    Ok(Arc::new(snapshot))
}

#[derive(Debug, thiserror::Error)]
pub enum ClientAccessLoadError {
    #[error("failed to read client access file `{path}`: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse client access file `{path}`: {source}")]
    Parse {
        path: String,
        source: serde_yml::Error,
    },
    #[error("failed to validate client access file `{path}`: {source}")]
    Validate {
        path: String,
        source: ClientAccessSnapshotError,
    },
}

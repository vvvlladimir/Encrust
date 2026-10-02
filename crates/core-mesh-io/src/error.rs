use std::path::PathBuf;

/// Why a mesh could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum MeshIoError {
    #[error("cannot read {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{path} is not valid {format}: {reason}")]
    Malformed {
        path: PathBuf,
        format: &'static str,
        reason: String,
    },

    #[error("texture {name} cannot be decoded: {reason}")]
    UndecodableTexture { name: String, reason: String },

    #[error("no loader registered for extension {0:?}")]
    UnsupportedExtension(String),

    #[error("{0} import is not implemented yet")]
    Unimplemented(&'static str),
}

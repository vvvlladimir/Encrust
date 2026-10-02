use std::path::PathBuf;

/// Why a profile could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("cannot read {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("cannot parse {path}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("cannot write {path}")]
    Serialise {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },

    #[error("{field} must be greater than zero, got {value}")]
    NonPositive { field: &'static str, value: f32 },

    #[error("{field} cannot be negative, got {value}")]
    Negative { field: &'static str, value: f32 },

    #[error("tuning for printer {printer}: {field} must be greater than zero, got {value}")]
    NonPositiveTuning {
        printer: String,
        field: &'static str,
        value: f32,
    },

    #[error("there is no {kind} called {id} in the catalogue")]
    Unknown { kind: &'static str, id: String },

    #[error("this platform has nowhere to keep your profiles; set {variable}")]
    NoUserDir { variable: &'static str },

    #[error("{id} is not a usable profile name: use letters, digits and dashes")]
    BadId { id: String },

    #[error("{field} must be at least {minimum}, got {value}")]
    TooFew {
        field: &'static str,
        minimum: u32,
        value: u32,
    },

    #[error("{smaller} ({smaller_value}) must not exceed {larger} ({larger_value})")]
    OutOfOrder {
        smaller: &'static str,
        smaller_value: f32,
        larger: &'static str,
        larger_value: f32,
    },
}

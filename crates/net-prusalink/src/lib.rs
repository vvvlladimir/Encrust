//! Client for `PrusaLink`, the HTTP API an Original Prusa machine serves on the local
//! network. Protocol details live in `docs/formats/prusalink.md`.

mod digest;
mod error;
mod link;
mod session;
mod upload;

pub use error::PrusaLinkError;
pub use link::{Auth, DEFAULT_USER, Link, Version};
pub use upload::{probe, start_print, upload};

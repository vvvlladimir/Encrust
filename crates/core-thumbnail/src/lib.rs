//! The shaded picture of a plate that a sliced file carries as its thumbnail.
//!
//! Rendering is done in software, on the CPU, so that the CLI and the window produce the
//! same image and no core crate has to know about a graphics API; see
//! `docs/decisions/0048-thumbnails-are-rendered-in-software.md`.

mod image;
mod render;
mod settings;

pub use image::Thumbnail;
pub use render::{Part, render};
pub use settings::{Rgb, ThumbnailSettings, rgb565};

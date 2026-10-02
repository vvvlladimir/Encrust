//! The Prusa `.sl1` and `.sl1s` containers: a zip of two settings files, two previews
//! and one greyscale PNG per layer. Layout is in `docs/formats/sl1.md`.

mod config;
#[cfg(test)]
mod fixtures;
mod reader;
mod writer;

pub use reader::{OpenSl1, Sl1Reader};
pub use writer::{EncodedLayer, Sl1Flavour, Sl1Sink, Sl1Writer};

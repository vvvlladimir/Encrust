//! Elegoo `.goo` output. Format details live in `docs/formats/goo.md`.

#[cfg(test)]
mod fixtures;
mod header;
mod layer;
mod reader;
mod rle;
mod writer;

pub use reader::{GooReader, OpenGoo};
pub use rle::{EncodedLayer, checksum, decode};
pub use writer::{GooSink, GooWriter};

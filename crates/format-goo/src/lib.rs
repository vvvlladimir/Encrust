//! Elegoo `.goo` output. Format details live in `docs/formats/goo.md`.

#[cfg(test)]
mod fixtures;
mod header;
mod layer;
mod reader;
mod rle;
mod rle5;
mod v5;
mod version;
mod writer;

pub use reader::{GooReader, OpenGoo};
pub use rle::{EncodedLayer, checksum, decode};
pub use rle5::{EncodedHalves, decode_half, join_halves};
pub use v5::{Goo5Sink, Goo5Writer};
pub use version::GooVersion;
pub use writer::{GooSink, GooWriter};

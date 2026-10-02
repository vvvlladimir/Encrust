//! Version 3 of the container: a layer is a list of vertical lines, each six bytes, and
//! everything in it but the magic is big-endian. See `docs/formats/creality.md`.

mod blocks;
mod lines;
mod reader;
mod writer;

pub use lines::{EncodedLayer, decode};
pub use reader::{CxdlpReader, OpenCxdlp};
pub use writer::{CxdlpSink, CxdlpWriter};

//! The zip of greyscale PNGs a Chitu board runs as gcode: one `run.gcode` program, two
//! previews and one eight-bit image per layer. See `docs/formats/gcode-zip.md`.

#[cfg(test)]
mod fixtures;
mod gcode;
mod reader;
mod writer;

pub use reader::{GcodeZipReader, OpenGcodeZip, claims};
pub use writer::{EncodedLayer, GcodeZipSink, GcodeZipWriter};

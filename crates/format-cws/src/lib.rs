//! The `.cws` archive: one `slice.conf` of settings, one eight-bit PNG per layer, and a
//! gcode program the board runs them with. See `docs/formats/cws.md`.

mod conf;
#[cfg(test)]
mod fixtures;
mod gcode;
mod reader;
mod writer;

pub use reader::{CwsReader, OpenCws, claims};
pub use writer::{CwsSink, CwsWriter, EncodedLayer};

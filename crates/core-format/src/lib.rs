//! What every sliced-file format has in common: the job being printed, the traits a
//! writer implements, and the byte-level field writer they all serialise through.
//!
//! A concrete format lives in its own crate beside this one; see `format-goo`.

mod de;
mod error;
mod exposure;
#[cfg(test)]
mod fixtures;
mod job;
mod png;
mod reader;
mod rgb15;
mod rle7;
mod ser;
mod timestamp;
mod writer;

pub use de::{MAX_ENTRY_BYTES, ReadSeek, Reads, read_entry};
pub use error::FormatError;
pub use exposure::{ExposurePlan, ExposureRange};
pub use job::PrintJob;
pub use png::{decode_grey, encode_colour, encode_grey, png_shape};
pub use reader::{
    LayerEntry, MAX_PANEL_PX, OpenFile, SlicedFile, SlicedFileReader, layer_in_range,
    panel_in_range,
};
pub use rgb15::{PREVIEW_HEADER_BYTES, PREVIEW_SIZES_PX, encode_rgb15, write_preview};
pub use rle7::{Rle7Layer, decode_rle7};
pub use ser::{Fields, WriteSeek};
pub use writer::{LayerSink, SlicedFileWriter, WRITE_BUFFER_BYTES, validate};

// How an exposure follows the layer height is a property of the resin, so it lives with
// the resin profile; every writer scales through it.
pub use printer_profiles::exposure_for_mm;

// Runs are the format-neutral currency of a rasterised layer; they live in `core-raster`
// so every sliced-file format speaks the same ones.
pub use core_raster::Run;

// The plan is the shape of the stack a writer records; it lives in `core-slicer` because
// that is what cuts to it.
pub use core_slicer::LayerPlan;

// The thumbnail is rendered before a job starts and carried by it, so a format crate
// needs the type but not the renderer.
pub use core_thumbnail::{Rgb, Thumbnail, rgb565};

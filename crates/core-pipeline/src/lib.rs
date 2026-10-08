//! The stage every front end shares: a cut stack rasterised into a printable file, and a
//! printable file read back or written again in another container.
//!
//! What is *above* this — which model to orient, hollow or support — belongs to the front
//! end that asked. What is below is the format writers and readers. See
//! `docs/architecture.md`.

mod convert;
mod error;
mod fold;
mod format;
mod panel;
mod read;
mod write;

pub use convert::{Converted, Converting, convert, convert_to};
pub use error::PipelineError;
pub use fold::{Folded, Tolerance, fold_group};
pub use format::{
    AnycubicFlavour, AnycubicVersion, CbddlpFlavour, CtbVersion, CxdlpVersion, Sl1Flavour,
    SlicedFormat,
};
pub use panel::{PanelOverrides, raster_settings};
pub use read::{Opened, open, open_file, reads_sliced_file};
pub use write::{Observer, Writing, Written, measure, write, write_to};

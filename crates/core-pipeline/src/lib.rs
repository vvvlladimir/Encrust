//! The stage every front end shares: a cut stack rasterised into a printable file.
//!
//! What is *above* this — which model to orient, hollow or support — belongs to the front
//! end that asked. What is below is the format writers and readers. See
//! `docs/architecture.md`.

mod error;
mod fold;
mod format;
mod panel;
mod read;
mod write;

pub use error::PipelineError;
pub use fold::{Folded, Tolerance, fold_group};
pub use format::SlicedFormat;
pub use panel::{PanelOverrides, raster_settings};
pub use read::{Opened, open, open_file, reads_sliced_file};
pub use write::{Observer, Writing, Written, measure, write, write_to};

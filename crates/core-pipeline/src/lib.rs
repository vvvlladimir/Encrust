//! The stage every front end shares: a cut stack rasterised into a file, and files read or
//! rewritten. Orienting and supporting belong to the front end; see `docs/architecture.md`.

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
    AnycubicFlavour, AnycubicVersion, CbddlpFlavour, CtbVersion, CxdlpVersion, PwszFlavour,
    Sl1Flavour, SlicedFormat,
};
pub use panel::{PanelOverrides, raster_settings};
pub use read::{Opened, open, open_file, reads_sliced_file};
pub use write::{Observer, Writing, Written, measure, write, write_to};

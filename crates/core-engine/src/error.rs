use core_pipeline::PipelineError;
use core_plate::PlateError;
use core_raster::RasterError;
use core_slicer::SliceError;

/// What can stop a plate on its way to a printable file.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("nothing on the plate to slice")]
    EmptyPlate,

    #[error("the panel cannot produce a usable mask")]
    Panel(#[source] RasterError),

    #[error("cannot work out the layers")]
    Plan(#[from] SliceError),

    #[error(transparent)]
    Pipeline(#[from] PipelineError),

    #[error("cannot slice the model to work out where it needs supports")]
    Supports(#[source] SliceError),

    #[error("cannot find an orientation for the model")]
    Orient(#[from] PlateError),

    #[error("the project names no printer to slice for")]
    NoPrinter,

    #[error("the project names no resin to slice with")]
    NoResin,

    #[error("the project's printer or resin is not among the profiles here")]
    NotHere(#[from] printer_profiles::ProfileError),
}

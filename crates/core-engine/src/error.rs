use core_pipeline::PipelineError;
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
}

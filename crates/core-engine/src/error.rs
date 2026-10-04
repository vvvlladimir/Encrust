use core_pipeline::PipelineError;
use core_raster::RasterError;
use core_slicer::SliceError;
use core_volume::VolumeError;

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

    #[error("the project names no printer to slice for")]
    NoPrinter,

    #[error("the project names no resin to slice with")]
    NoResin,

    #[error("cannot hollow {object} again")]
    Hollow {
        object: String,
        #[source]
        source: VolumeError,
    },
}

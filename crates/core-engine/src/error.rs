use core_geometry::Vec3;
use core_pipeline::PipelineError;
use core_plate::PlateError;
use core_raster::RasterError;
use core_slicer::SliceError;
use core_supports::Refused;
use core_volume::VolumeError;

use crate::stand::merge_refusal;

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

    #[error("cannot drill the drain holes a search tried")]
    Drill(#[source] VolumeError),

    #[error("cannot find an orientation for the model")]
    Orient(#[from] PlateError),

    #[error("the project names no printer to slice for")]
    NoPrinter,

    #[error("the project names no resin to slice with")]
    NoResin,

    #[error("the project's printer or resin is not among the profiles here")]
    NotHere(#[from] printer_profiles::ProfileError),

    #[error("no support has its contact within half a millimetre of {at}")]
    NoSupportNear { at: Vec3 },

    #[error("the points of the merge at {at} name one support, and a merge needs two")]
    OneSupportToMerge { at: Vec3 },

    #[error("the supports of the merge at {at} cannot be merged: {}", merge_refusal(*why))]
    MergeRefused { at: Vec3, why: Refused },
}

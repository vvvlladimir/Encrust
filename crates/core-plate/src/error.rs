use core_slicer::SliceError;
use thiserror::Error;

/// What can stop a model being placed on the plate.
#[derive(Debug, Error)]
pub enum PlateError {
    #[error("the mesh has no face with any area, so it has no orientation")]
    NothingToOrient,
    #[error("cannot cut the model to measure its cross-section")]
    Slice(#[from] SliceError),
}

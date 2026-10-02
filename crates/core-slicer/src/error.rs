/// Why a mesh could not be sliced.
#[derive(Debug, thiserror::Error)]
pub enum SliceError {
    #[error("layer height must be greater than zero, got {0}")]
    NonPositiveLayerHeight(f32),

    #[error("the cusp target must be greater than zero, got {0}")]
    NonPositiveCusp(f32),

    #[error("the layer range runs from {min_mm} mm down to {max_mm} mm")]
    UpsideDownRange { min_mm: f32, max_mm: f32 },

    #[error("mesh has no triangles")]
    EmptyMesh,
}

/// Why a layer could not be rasterised.
#[derive(Debug, thiserror::Error)]
pub enum RasterError {
    #[error("mask resolution must be non-zero, got {width}x{height}")]
    ZeroResolution { width: u32, height: u32 },

    #[error("a panel of {width}x{height} has more pixels than a layer can index")]
    PanelTooLarge { width: u32, height: u32 },

    #[error("pixel pitch must be greater than zero, got {0} mm")]
    NonPositivePixelPitch(f32),
}

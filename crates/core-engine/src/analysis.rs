use core_geometry::Scalar;
use core_slicer::SliceSettings;

/// The layer height, mm, a model is sliced at to find where it needs holding up and which
/// pockets it traps, whatever the printer (ADR 0471).
pub const ANALYSIS_LAYER_MM: Scalar = 0.05;

/// The slicing a model is analysed with: [`ANALYSIS_LAYER_MM`] and the slicer's other defaults.
pub fn analysis_slicing() -> SliceSettings {
    SliceSettings {
        layer_height: ANALYSIS_LAYER_MM,
        ..SliceSettings::default()
    }
}

/// Why a sliced file could not be written or read back.
///
/// One enum for every format: each variant describes a mismatch between the job and the
/// layers it was handed, which no format is allowed to paper over.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("cannot write output")]
    Io(#[from] std::io::Error),

    #[error("job has no layers")]
    EmptyJob,

    #[error(
        "the printer panel is {printer_width}x{printer_height} but the masks are {raster_width}x{raster_height}"
    )]
    PanelMismatch {
        printer_width: u32,
        printer_height: u32,
        raster_width: u32,
        raster_height: u32,
    },

    #[error(
        "layer {index} is {width}x{height} but the printer panel is {expected_width}x{expected_height}"
    )]
    ResolutionMismatch {
        index: usize,
        width: u32,
        height: u32,
        expected_width: u32,
        expected_height: u32,
    },

    #[error("the header promised {expected} layers but {written} were written")]
    LayerCountMismatch { expected: u32, written: u32 },

    #[error("run-length data is corrupt at byte {offset}")]
    MalformedRun { offset: usize },

    #[error("{printer} does not read the per-layer tables, so exposure cannot vary by height")]
    PerLayerUnsupported { printer: String },

    #[error("an exposure band from {from_mm} mm asks for {exposure_s} s")]
    NonPositiveExposure { from_mm: f32, exposure_s: f32 },

    #[error(
        "{printer} steps the plate by the header's layer height, so a stack of layers of different thicknesses cannot be printed"
    )]
    VariableHeightUnsupported { printer: String },

    #[error("{format} version {version} is not supported")]
    UnsupportedVersion { format: &'static str, version: u32 },

    #[error("a .{format} states one {field} for the whole stack, so it cannot vary by height")]
    FixedForWholeStack {
        format: &'static str,
        field: &'static str,
    },

    #[error("a block at {offset} is past the end of a {end}-byte file")]
    AddressPastEnd { offset: u64, end: u64 },

    #[error("a header field claims {count} {what}, more than the {available} bytes behind it")]
    ImpossibleCount {
        what: &'static str,
        count: u64,
        available: u64,
    },

    #[error("the header states a {width_px}x{height_px} panel, which no printer has")]
    PanelTooLarge { width_px: u32, height_px: u32 },

    #[error("this is not a {format}: the magic reads {found:#010x}")]
    NotThisFormat { format: &'static str, found: u32 },

    #[error("no reader recognises this file")]
    UnknownContainer,

    #[error("the file is missing {what}")]
    Missing { what: String },

    #[error("layer {index} was asked for in a file of {layer_count} layers")]
    NoSuchLayer { index: u32, layer_count: u32 },

    #[error("{what} could not be encoded: {reason}")]
    Encoding { what: &'static str, reason: String },
}

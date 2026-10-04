use std::path::PathBuf;

use core_format::FormatError;
use core_geometry::Scalar;
use core_raster::RasterError;
use core_slicer::SliceError;

/// What can stop a stack on its way from a mesh to a printable file.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("cannot slice the model")]
    Slice(#[from] SliceError),

    #[error("cannot rasterise the layer at z = {z:.3} mm")]
    Raster {
        /// Height of the layer that failed, millimetres above the plate.
        z: Scalar,
        #[source]
        source: RasterError,
    },

    #[error("cannot create {}", path.display())]
    Create {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("cannot write {name}")]
    Write {
        /// What the file being written is called, without its directory or extension.
        name: String,
        #[source]
        source: FormatError,
    },

    #[error("cannot read {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("cannot open {}", path.display())]
    Open {
        path: PathBuf,
        #[source]
        source: FormatError,
    },

    #[error(
        "the file's masks are {} x {} px and the printer's panel is {} x {} px; a mask is not resampled",
        file_px.0, file_px.1, printer_px.0, printer_px.1
    )]
    PanelMismatch {
        file_px: (u32, u32),
        printer_px: (u32, u32),
    },

    #[error(
        "the file's masks were drawn for a {:.2} x {:.2} mm panel and the printer's is {:.2} x {:.2} mm; the print would come out at the wrong scale",
        file_mm.0, file_mm.1, printer_mm.0, printer_mm.1
    )]
    PanelSizeMismatch {
        /// Panel size the file records, millimetres.
        file_mm: (f32, f32),
        /// Panel size of the printer converted to, millimetres.
        printer_mm: (f32, f32),
    },

    #[error("the file's layers are not all one height, which a conversion cannot carry yet")]
    VaryingHeights,

    #[error("cannot decode layer {layer}")]
    Decode {
        /// Index of the layer, counted from zero.
        layer: usize,
        #[source]
        source: FormatError,
    },
}

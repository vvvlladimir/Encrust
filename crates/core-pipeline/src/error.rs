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

    #[error("cannot write {}", path.display())]
    Write {
        path: PathBuf,
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
}

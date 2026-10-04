use core_geometry::{Scalar, Vec3};
use thiserror::Error;

/// What can go wrong building a field or combining two of them.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum VolumeError {
    #[error("a field needs a mesh with at least one face")]
    EmptyMesh,

    #[error("the voxel size must be a positive length, got {0} mm")]
    BadVoxelSize(Scalar),

    #[error("the band must be at least one voxel wide, got {0} voxels")]
    BadBand(Scalar),

    #[error("the isosurface must be at a finite distance from the mesh, got {0} mm")]
    BadIsoLevel(Scalar),

    #[error("the wall must be a positive thickness, got {0} mm")]
    BadThickness(Scalar),

    #[error("a model scaled by {0} has an axis with no size to hollow")]
    BadScale(Vec3),

    #[error("a relief needs an amplitude to move the surface by, got {0} mm")]
    BadAmplitude(Scalar),

    #[error("the texture covers {mapped} of the mesh's {faces} faces")]
    UnmappedMesh { faces: usize, mapped: usize },

    #[error("the map is textured from {wanted} images and {given} were handed over")]
    MissingTexture { wanted: usize, given: usize },

    #[error("the infill cell must be a positive length, got {0} mm")]
    BadCell(Scalar),

    #[error("the infill density must be between 0 and 1, got {0}")]
    BadDensity(Scalar),

    #[error("drain hole {index} has no width, no depth or no direction to be drilled in")]
    BadDrain { index: usize },

    #[error("drain channel {index} needs a width and at least two points to be dug along")]
    BadChannel { index: usize },

    #[error(
        "the fields sit on different lattices: {left_voxel_mm} mm voxels against {right_voxel_mm} mm"
    )]
    GridMismatch {
        left_voxel_mm: Scalar,
        right_voxel_mm: Scalar,
    },

    #[error("moving the surface by {asked_mm} mm needs a band wider than {band_mm} mm")]
    OutsideBand { asked_mm: Scalar, band_mm: Scalar },

    #[error(
        "a {voxel_mm} mm lattice over this model needs {} MB, past the {} MB it may take; {fits_at_mm} mm fits",
        needed_bytes >> 20,
        budget_bytes >> 20
    )]
    TooFine {
        needed_bytes: usize,
        budget_bytes: usize,
        /// The lattice that was asked for, in millimetres.
        voxel_mm: Scalar,
        /// The coarsest lattice that would have fit, in millimetres.
        fits_at_mm: Scalar,
    },
}

//! The shell a hollowed model carries, as the file records it: the wall asked for and what the
//! lattice gave (ADR 0191).

use std::ops::Range;

use core_geometry::{Mat3, Scalar, Vec3};
use core_volume::{HollowMode, HollowSettings, InfillSettings, Shell};
use serde::{Deserialize, Serialize};

/// A shell as the run left it: the wall it was asked for, and what came out of the
/// lattice. Everything here is measured, not asked for again, so the file opens as the
/// plate that was saved (ADR 0191).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuiltCavity {
    pub wall: Cavity,
    /// Which faces of the shell bound the space the resin fills.
    pub cavity_faces: Range<usize>,
    pub cavity_mm3: Scalar,
    /// The lattice the cavity came out on, millimetres, and whether a memory budget made
    /// it coarser than the precision asked for.
    pub voxel_mm: Scalar,
    pub coarsened: bool,
    /// The size the model stood at, which the wall was measured under; see [`Self::shape`].
    pub scale: Vec3,
    /// That size whole, written only where the model was stretched along the plate too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stretched: Option<Mat3>,
}

impl BuiltCavity {
    /// The shell as the file records it: the wall it was asked for and what the lattice gave.
    pub fn of(shell: &Shell) -> Self {
        let (scale, stretched) = Self::keys_of(shell.shape);
        Self {
            wall: Cavity::of(&shell.settings),
            cavity_faces: shell.cavity.clone(),
            cavity_mm3: shell.cavity_mm3,
            voxel_mm: shell.voxel_mm,
            coarsened: shell.coarsened,
            scale,
            stretched,
        }
    }

    /// What the wall was measured under (`Transform::unturned`), however the file wrote it.
    pub fn shape(&self) -> Mat3 {
        self.stretched
            .unwrap_or_else(|| Mat3::from_diagonal(self.scale.abs()))
    }

    /// `shape` as the two keys write it: its diagonal, and itself where that is not all.
    pub fn keys_of(shape: Mat3) -> (Vec3, Option<Mat3>) {
        let diagonal = Vec3::new(shape.x_axis.x, shape.y_axis.y, shape.z_axis.z);
        let stretched = (shape != Mat3::from_diagonal(diagonal)).then_some(shape);
        (diagonal, stretched)
    }
}

/// The faces of a printed surface of `faces` faces that lie inside the model: its cavity,
/// and the lattice standing in it, which the surface counts after it.
pub fn inner_faces(cavity: &Range<usize>, faces: usize) -> Range<usize> {
    cavity.start..faces.max(cavity.end)
}

/// The wall a model was hollowed to, as the Hollow panel shows it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Cavity {
    pub thickness_mm: Scalar,
    pub mode: HollowMode,
    pub precision: Scalar,
    pub infill: Option<InfillSettings>,
    /// The way to the plate the floor was opened toward; absent for a closed shell, and in
    /// a file from before ADR 0313.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_toward: Option<Vec3>,
    /// Where the wall was kept solid under, millimetres; absent before ADR 0404.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solid_under_mm: Option<Scalar>,
}

impl Cavity {
    /// The wall `asked` builds, without what the model laid over it.
    pub fn of(asked: &HollowSettings) -> Self {
        Self {
            thickness_mm: asked.thickness_mm,
            mode: asked.mode,
            precision: asked.precision,
            infill: asked.infill,
            open_toward: asked.open_toward,
            solid_under_mm: asked.solid_under_mm,
        }
    }

    /// What a run is asked for to build this wall again, before the model lays its own
    /// blockers and channels over it.
    pub fn settings(&self) -> HollowSettings {
        HollowSettings {
            thickness_mm: self.thickness_mm,
            mode: self.mode,
            precision: self.precision,
            infill: self.infill,
            open_toward: self.open_toward,
            solid_under_mm: self.solid_under_mm,
            ..HollowSettings::default()
        }
    }
}

use core_geometry::{
    Bvh, MendMethod, MendSettings, Mended, Mesh, Scalar, diagnose, mend, self_intersections,
};

use crate::boolean::{Combined, solid_of};
use crate::build::{FieldSettings, build};
use crate::cancel::Cancel;
use crate::error::VolumeError;
use crate::extract::extract;
use crate::hollow::coarsening;
use crate::sign::SignMode;

/// The lattice a wrap falls back to the field on unless asked otherwise, mm.
pub const DEFAULT_WRAP_MM: Scalar = 0.2;

/// Which repair to run, the ones a field may be needed for among them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairMethod {
    /// One of the repairs on the mesh alone.
    Mend(MendMethod),
    /// The mesh repaired and wrapped in one outer surface.
    Wrap,
    /// Repaired, wrapped and repaired again.
    Deep,
}

/// The lengths a repair works to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RepairSettings {
    pub mend: MendSettings,
    /// The lattice a wrap falls back to, mm.
    pub wrap_mm: Scalar,
}

impl Default for RepairSettings {
    fn default() -> Self {
        Self {
            mend: MendSettings::default(),
            wrap_mm: DEFAULT_WRAP_MM,
        }
    }
}

/// A mesh as a repair left it, and what was done to it.
#[derive(Debug, Clone, PartialEq)]
pub struct Repaired {
    pub mesh: Mesh,
    /// What the last repair on the mesh alone changed.
    pub mended: Mended,
    /// The lattice a wrap was sampled on, mm; `None` where it was cut exactly or not wrapped.
    pub voxel_mm: Option<Scalar>,
    /// False where no wrap was asked for, or the one the field gave would have left the
    /// model worse, so only the repair stands.
    pub wrapped: bool,
}

/// `mesh` repaired by `method`; see `docs/design/mesh-repair.md`.
pub fn repair(
    mesh: &Mesh,
    method: RepairMethod,
    settings: RepairSettings,
    cancel: Cancel<'_>,
) -> Result<Repaired, VolumeError> {
    if mesh.faces.is_empty() {
        return Err(VolumeError::EmptyMesh);
    }
    let mended_alone = |mesh: Mesh, method: MendMethod| {
        let mut mesh = mesh;
        let mended = mend(&mut mesh, method, settings.mend);
        Repaired {
            mesh,
            mended,
            voxel_mm: None,
            wrapped: false,
        }
    };
    match method {
        RepairMethod::Mend(method) => Ok(mended_alone(mesh.clone(), method)),
        RepairMethod::Wrap => wrap(mesh, settings, cancel),
        RepairMethod::Deep => {
            let wrapped = wrap(mesh, settings, cancel)?;
            Ok(Repaired {
                voxel_mm: wrapped.voxel_mm,
                wrapped: wrapped.wrapped,
                ..mended_alone(wrapped.mesh, MendMethod::Repair)
            })
        }
    }
}

/// `mesh` repaired, then wrapped in one outer surface: its shells unioned exactly, or where
/// the engine cannot read it or it still crosses itself, sampled into a winding field, which
/// rounds sharp edges to `settings.wrap_mm`.
pub fn wrap(
    mesh: &Mesh,
    settings: RepairSettings,
    cancel: Cancel<'_>,
) -> Result<Repaired, VolumeError> {
    if mesh.faces.is_empty() {
        return Err(VolumeError::EmptyMesh);
    }
    let mut repaired = mesh.clone();
    let mended = mend(&mut repaired, MendMethod::Repair, settings.mend);
    if cancel.asked() {
        return Err(VolumeError::Cancelled);
    }
    let wrapped = if let Some(mesh) = exact(&repaired) {
        Some(Combined {
            mesh,
            voxel_mm: None,
        })
    } else {
        through_field(&repaired, settings.wrap_mm, cancel)?
    };
    Ok(match wrapped {
        Some(wrapped) => Repaired {
            mesh: wrapped.mesh,
            mended,
            voxel_mm: wrapped.voxel_mm,
            wrapped: true,
        },
        None => Repaired {
            mesh: repaired,
            mended,
            voxel_mm: None,
            wrapped: false,
        },
    })
}

/// The field's surface of `repaired`, or `None` where it would leave the model worse.
fn through_field(
    repaired: &Mesh,
    wrap_mm: Scalar,
    cancel: Cancel<'_>,
) -> Result<Option<Combined>, VolumeError> {
    let sampled = sampled(repaired, wrap_mm, cancel)?;
    if sampled.mesh.faces.is_empty() {
        return Err(VolumeError::NothingLeft);
    }
    Ok(better(&sampled.mesh, repaired).then_some(sampled))
}

/// Whether a surface out of the field is worth having over `before`: sound, crossing itself
/// less, and branching no more. The lattice can pinch a thin part into a branch.
fn better(sampled: &Mesh, before: &Mesh) -> bool {
    let crossings = |mesh: &Mesh| self_intersections(mesh, &Bvh::build(mesh)).len();
    let (found, was) = (diagnose(sampled), diagnose(before));
    found.is_sound()
        && found.non_manifold_edges <= was.non_manifold_edges
        && crossings(sampled) < crossings(before)
}

/// The shells unioned by the exact engine, or `None` where it refuses them or leaves a
/// surface that is open or still crosses itself.
fn exact(mesh: &Mesh) -> Option<Mesh> {
    let solid = solid_of(mesh).ok()?;
    if solid.is_empty() {
        return None;
    }
    let united = solid.mesh();
    let crosses = !self_intersections(&united, &Bvh::build(&united)).is_empty();
    (diagnose(&united).is_sound() && !crosses).then_some(united)
}

/// The surface of the winding field, which reads through holes and crossings alike.
fn sampled(mesh: &Mesh, voxel_mm: Scalar, cancel: Cancel<'_>) -> Result<Combined, VolumeError> {
    let bvh = Bvh::build(mesh);
    let (field, voxel_mm) = coarsening(voxel_mm, |voxel_mm| {
        let settings = FieldSettings {
            voxel_mm,
            sign: SignMode::Winding,
            ..FieldSettings::default()
        };
        build(mesh, &bvh, &settings, cancel)
    })?;
    let mesh = extract(&field, cancel);
    if cancel.asked() {
        return Err(VolumeError::Cancelled);
    }
    Ok(Combined {
        mesh,
        voxel_mm: Some(voxel_mm),
    })
}

#[cfg(test)]
mod tests;

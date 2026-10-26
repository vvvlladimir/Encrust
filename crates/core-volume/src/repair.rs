use core_geometry::{
    Bvh, MendMethod, MendSettings, Mended, Mesh, Shell, diagnose, list_shells, mend,
    self_intersections,
};

use serde::{Deserialize, Serialize};

use crate::boolean::solid_of;
use crate::cancel::Cancel;
use crate::error::VolumeError;

/// Which repair to run, on the mesh alone or wrapping it in one surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepairMethod {
    /// One of the repairs on the mesh alone.
    Mend(MendMethod),
    /// The mesh repaired and wrapped in one outer surface.
    Wrap,
    /// Repaired, wrapped and repaired again.
    Deep,
}

/// The lengths a repair works to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct RepairSettings {
    pub mend: MendSettings,
}

impl RepairMethod {
    /// Whether the method wraps the mesh in one outer surface.
    pub fn wraps(self) -> bool {
        matches!(self, Self::Wrap | Self::Deep)
    }
}

impl Default for RepairMethod {
    fn default() -> Self {
        Self::Mend(MendMethod::Repair)
    }
}

/// A mesh as a repair left it, and what was done to it.
#[derive(Debug, Clone, PartialEq)]
pub struct Repaired {
    pub mesh: Mesh,
    /// What the repairs on the mesh alone changed, together.
    pub mended: Mended,
    /// False where no wrap was asked for, or the exact union could not give one, so only
    /// the repair stands.
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
            wrapped: false,
        }
    };
    match method {
        RepairMethod::Mend(method) => Ok(mended_alone(mesh.clone(), method)),
        RepairMethod::Wrap => wrap(mesh, settings, cancel),
        RepairMethod::Deep => {
            let wrapped = wrap(mesh, settings, cancel)?;
            // Unwrapped, the mesh is the first repair's own, which a second one leaves be.
            if !wrapped.wrapped {
                return Ok(wrapped);
            }
            let again = mended_alone(wrapped.mesh, MendMethod::Repair);
            Ok(Repaired {
                mended: wrapped.mended.then(again.mended),
                wrapped: true,
                ..again
            })
        }
    }
}

/// `mesh` repaired, then its shells unioned exactly into one outer surface; see
/// `docs/design/mesh-repair.md`.
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
    let crossings = self_intersections(&repaired, &Bvh::build(&repaired));
    let shells = list_shells(&repaired);
    // One sound shell crossing nothing is its own outer surface.
    let alone = shells.len() == 1 && crossings.is_empty() && diagnose(&repaired).is_sound();
    let united = (!alone && !crosses_itself(&shells, &crossings))
        .then(|| exact(&repaired))
        .flatten();
    let wrapped = alone || united.is_some();
    Ok(Repaired {
        mesh: united.unwrap_or(repaired),
        mended,
        wrapped,
    })
}

/// Whether a shell crosses itself, which the exact engine returns still crossing (ADR 0352).
fn crosses_itself(shells: &[Shell], crossings: &[[u32; 2]]) -> bool {
    if crossings.is_empty() {
        return false;
    }
    let mut shell_of = vec![0u32; shells.iter().map(|shell| shell.faces.len()).sum()];
    for (index, shell) in shells.iter().enumerate() {
        for &face in &shell.faces {
            shell_of[face as usize] = index as u32;
        }
    }
    crossings
        .iter()
        .any(|[one, other]| shell_of[*one as usize] == shell_of[*other as usize])
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

#[cfg(test)]
mod tests;

//! A model mended or repaired where it stands (`docs/design/document.md`).

use core_edit::{Edit, ObjectId};
use core_geometry::{
    MendSettings, Mesh, Region, Scalar, bridge, flip_faces, list_shells, remove_faces,
};
use core_volume::{Cancel, RepairMethod, RepairSettings, Repaired, repair};

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;

/// What is turned, bridged and dropped on a mesh by hand.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MendAsked {
    /// Faces from 0.
    pub flip_faces: Vec<usize>,
    /// Faces from 0.
    pub drop_faces: Vec<usize>,
    /// Shells from 1, in `list_shells` order.
    pub drop_shells: Vec<usize>,
    /// Pairs of open edges, each as its two vertices.
    pub bridge: Vec<[[u32; 2]; 2]>,
}

impl MendAsked {
    /// What a mend edit asks; `None` for any other edit.
    pub fn of(edit: &Edit) -> Option<Self> {
        let Edit::Mend {
            flip_faces,
            drop_faces,
            drop_shells,
            bridge,
            ..
        } = edit
        else {
            return None;
        };
        Some(Self {
            flip_faces: flip_faces.clone(),
            drop_faces: drop_faces.clone(),
            drop_shells: drop_shells.clone(),
            bridge: bridge.clone(),
        })
    }

    /// Whether nothing is asked.
    pub fn is_empty(&self) -> bool {
        self.flip_faces.is_empty()
            && self.drop_faces.is_empty()
            && self.drop_shells.is_empty()
            && self.bridge.is_empty()
    }
}

/// What a mend by hand did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HandMended {
    pub faces_flipped: usize,
    pub faces_added: usize,
    pub faces_dropped: usize,
    pub shells_dropped: usize,
}

/// Turns and bridges what `asked` names on `mesh`, then drops what it names; `object` names
/// the model in an error, which leaves `mesh` half done.
pub fn mended(
    mesh: &mut Mesh,
    asked: &MendAsked,
    object: ObjectId,
) -> Result<HandMended, ApplyError> {
    let faces = mesh.faces.len();
    let flipped = faces_named(&asked.flip_faces, faces, object)?;
    let mut dropped = faces_named(&asked.drop_faces, faces, object)?;
    let mut numbers = asked.drop_shells.clone();
    numbers.sort_unstable();
    numbers.dedup();
    let shells = list_shells(mesh);
    for &number in &numbers {
        let shell = (number.checked_sub(1).and_then(|at| shells.get(at)))
            .ok_or(ApplyError::NoSuchShell { object, number })?;
        for face in &shell.faces {
            dropped.set(*face as usize, true);
        }
    }
    let mut done = HandMended {
        faces_flipped: flip_faces(mesh, &flipped),
        shells_dropped: numbers.len(),
        ..HandMended::default()
    };
    for &[a, b] in &asked.bridge {
        done.faces_added += bridge(mesh, a, b).map_err(|error| ApplyError::NoBridge {
            object,
            edges: [a, b],
            reason: error.to_string(),
        })?;
    }
    done.faces_dropped = remove_faces(mesh, &dropped);
    Ok(done)
}

fn faces_named(numbers: &[usize], faces: usize, object: ObjectId) -> Result<Region, ApplyError> {
    match numbers.iter().find(|&&number| number >= faces) {
        Some(&number) => Err(ApplyError::NoSuchFace { object, number }),
        None => Ok(numbers.iter().copied().collect()),
    }
}

/// `mesh` repaired by `method` to the lengths `settings` give, `object` naming the model in
/// an error.
pub fn repaired(
    mesh: &Mesh,
    (method, settings): (RepairMethod, RepairSettings),
    cancel: Cancel<'_>,
    object: ObjectId,
) -> Result<Repaired, ApplyError> {
    repair(mesh, method, settings, cancel).map_err(|error| ApplyError::of_volume(object, error))
}

/// Repairs the mesh of `id` by `method`.
pub(crate) fn apply_repair(
    document: &mut Document,
    id: ObjectId,
    (method, (seam_mm, shell_mm2)): (RepairMethod, (Option<Scalar>, Option<Scalar>)),
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    let (_, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let settings = repair_settings(seam_mm, shell_mm2);
    let done = repaired(&meshes.source, (method, settings), cancel, id)?;
    // A wrap's surface stands whatever the repair under it did, as in the window's Repair.
    if done.wrapped || !done.mended.kept_as_found {
        document.reshape(id, done.mesh);
        if let Some(orientation) = done.mended.orientation {
            document.set_orientation(id, orientation);
        }
    }
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report: Report::Repaired {
            mended: done.mended,
            wrapped: done.wrapped,
        },
    })
}

/// A repair's lengths as `Edit::Repair` asks them, mm and mm², the window's where unset.
pub fn repair_settings(seam_mm: Option<Scalar>, shell_mm2: Option<Scalar>) -> RepairSettings {
    let window = RepairSettings::default().mend;
    RepairSettings {
        mend: MendSettings {
            seam_tolerance_mm: seam_mm.unwrap_or(window.seam_tolerance_mm),
            smallest_shell_mm2: shell_mm2.unwrap_or(window.smallest_shell_mm2),
        },
    }
}

/// Turns, bridges and drops what `asked` names on the mesh of `id`.
pub(crate) fn apply_mend(
    document: &mut Document,
    id: ObjectId,
    asked: &MendAsked,
) -> Result<Applied, ApplyError> {
    let (_, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let mut mesh = (*meshes.source).clone();
    let done = mended(&mut mesh, asked, id)?;
    document.reshape(id, mesh);
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report: Report::Mended(done),
    })
}

#[cfg(test)]
mod tests;

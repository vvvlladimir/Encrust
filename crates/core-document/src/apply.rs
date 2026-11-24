use core_edit::{Edit, ObjectId};
use core_engine::project::hollow_of;
use core_geometry::{Mesh, Scalar, Transform, Vec3};
use core_volume::Cancel;

use crate::arrange::apply_arrange;
use crate::combine::apply_combine;
use crate::cut::{CutAsked, CutModel, cut, pieces};
use crate::document::Document;
use crate::drain::apply_drains;
use crate::error::ApplyError;
use crate::hollow::apply_hollow;
use crate::label::{LabelAsked, apply_label};
use crate::orient::{OrientAsked, apply_orient};
use crate::piece::Role;
use crate::placement::{moved, sized_to, turned_to};
use crate::plate_contents::{delete, duplicate, mirrored};
use crate::relief::{ReliefAsked, apply_relief};
use crate::repair::{MendAsked, Mended, apply_mend, apply_repair};
use crate::split::{apply_part_off, apply_split};
use crate::supports::{SupportsAsked, apply_merge, apply_supports, stand_at};

/// What an edit did, for a front end to name, select and report it.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    pub removed: Vec<ObjectId>,
    /// Each object the edit made, with what it is to the one it came from.
    pub added: Vec<(ObjectId, Role)>,
    pub report: Report,
}

/// What an edit has to say beyond the objects it changed.
#[derive(Debug, Clone, PartialEq)]
pub enum Report {
    /// An object was cut in two.
    Cut {
        /// Cut loops that did not close, and were left uncapped.
        open_loops: usize,
        /// Pins, dowels or magnets put in, or pieces of the face a key runs round.
        placed: usize,
        /// Pieces of the cut face too small or thin to take a fixing.
        bare: usize,
        /// Why the halves were left plain when they were to be joined.
        unjoined: Option<String>,
        /// Pieces that did not come out sound and were mended.
        mended: usize,
    },
    /// An object was broken into its parts, or left whole where it was one.
    Split {
        parts: usize,
        /// Scraps too small to be parts, joined to the part they touch.
        joined: usize,
        /// Scraps too small to be parts and touching none, left out.
        dropped: usize,
    },
    /// A part was cut off an object.
    PartedOff,
    /// Objects were combined into one.
    Combined {
        /// The lattice it was sampled on, mm, where an object was not an exact solid.
        voxel_mm: Option<Scalar>,
    },
    /// A label was written on an object.
    Labelled,
    /// An object's mesh was repaired.
    Repaired {
        mended: core_geometry::Mended,
        /// Whether the mesh is the wrap's surface rather than the repair's alone.
        wrapped: bool,
    },
    /// An object's faces were turned, bridged or dropped by hand.
    Mended(Mended),
    /// A picture was pressed into an object's mesh.
    Relieved {
        /// The lattice it came out on, mm.
        voxel_mm: Scalar,
        /// Whether the memory budget made the lattice coarser than precision asked for.
        coarsened: bool,
    },
    /// An object was hollowed, the numbers kept as its own.
    Hollowed {
        /// Resin the cavity takes out, mm³.
        cavity_mm3: Scalar,
        /// The lattice it was cut on, mm.
        voxel_mm: Scalar,
        /// Whether the memory budget made the lattice coarser than precision asked for.
        coarsened: bool,
    },
    /// Holes were drilled into an object.
    Drained {
        /// Holes put in; one an earlier hole takes in adds none.
        holes: usize,
        /// Pockets of resin still trapped once the search's holes were in; 0 without a search.
        trapped: usize,
        /// What the search found and chose, plate mm where the object stood; none without one.
        searched: Option<core_engine::Drained>,
    },
    /// A channel was dug through an object.
    Channelled,
    /// Plugs were laid beside an object for its holes.
    Plugged {
        /// Plugs laid, one for each hole the gap left open.
        plugs: usize,
        /// Holes the gap closed at the mouth, which got none.
        closed: usize,
        /// Plugs that fit nowhere on the plate, left where they were laid.
        crowded: usize,
    },
    /// Supports were stood under an object.
    Supported {
        /// Contacts put in, by hand or found.
        placed: usize,
        /// Trees grown from the object's contacts, those it already had among them.
        trees: usize,
        /// Every contact of the object, plate mm.
        contacts_mm: Vec<Vec3>,
        /// Tips a tree carries; fewer than the contacts where one found no room.
        standing: usize,
        /// The contacts no tree carries, plate mm, each with why.
        unheld: Vec<(Vec3, core_supports::Unstood)>,
    },
    /// Supports of an object were joined into one.
    Merged,
    /// An object was moved, turned or sized where it stands.
    Placed,
    /// The plate's objects were spread over it.
    Arranged {
        /// Objects standing clear of one another on the plate.
        placed: usize,
        /// Objects that found no room, left where they were.
        left_out: Vec<ObjectId>,
    },
    /// An object was stood the way it prints best.
    Oriented {
        /// The turn it now stands at, degrees about X, then Y, then Z of the plate.
        rotate_deg: Vec3,
        /// Whether the pose sets it on the plate.
        on_plate: bool,
        /// The pose's largest section and its overhang, mm².
        peak_mm2: Scalar,
        overhang_mm2: Scalar,
        /// Every pose the search ranked, best first.
        ranked: Vec<crate::orient::Ranked>,
    },
    /// An object was deleted or copied.
    Contents,
}

/// Carries out an edit that remakes or builds on an object's surface: a repair, a relief, a
/// hollow, holes and supports.
fn apply_to_the_surface(
    document: &mut Document,
    edit: &Edit,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    match *edit {
        Edit::Repair {
            object,
            method,
            seam_tolerance_mm,
            smallest_shell_mm2,
        } => {
            let limits = (seam_tolerance_mm, smallest_shell_mm2);
            apply_repair(document, object, (method, limits), cancel)
        }
        Edit::Mend { object, .. } => {
            let asked = MendAsked::of(edit).ok_or(ApplyError::NoObject(object))?;
            apply_mend(document, object, &asked)
        }
        Edit::Relief { object, .. } => {
            let asked = ReliefAsked::of(edit).ok_or(ApplyError::NoObject(object))?;
            apply_relief(document, object, &asked, cancel)
        }
        Edit::Hollow {
            object,
            ref settings,
            ref keep_solid,
            keep_solid_reach_mm,
        } => {
            let kept = (keep_solid.as_slice(), keep_solid_reach_mm);
            apply_hollow(document, object, (settings, kept), cancel)
        }
        Edit::Drain { object, .. } | Edit::Channel { object, .. } | Edit::Plugs { object, .. } => {
            apply_drains(document, edit, object, cancel)
        }
        Edit::Supports { object, .. } => {
            let asked = SupportsAsked::of(edit).ok_or(ApplyError::NoObject(object))?;
            apply_supports(document, object, &asked, cancel)
        }
        Edit::MergeSupports {
            object,
            ref near_mm,
        } => apply_merge(document, object, near_mm),
        _ => Err(ApplyError::NoObject(
            edit.object().unwrap_or(ObjectId::UNSET),
        )),
    }
}

/// Carries out an edit that makes new objects of others: a cut, a split, a part-off, a
/// combination or a label.
fn apply_modelling(
    document: &mut Document,
    edit: &Edit,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    match *edit {
        Edit::Cut {
            object,
            axis,
            at_mm,
            keep,
            joints,
            lay_flat,
        } => {
            let asked = CutAsked {
                axis,
                at_mm,
                keep,
                joints,
                lay_flat,
            };
            apply_cut(document, object, &asked)
        }
        Edit::Split {
            object,
            smallest_mm3,
        } => apply_split(document, object, smallest_mm3, cancel),
        Edit::PartOff { object, at_mm } => apply_part_off(document, object, at_mm, cancel),
        Edit::Combine {
            ref objects,
            op,
            toward,
        } => apply_combine(document, objects, op, toward, cancel),
        Edit::Label { object, .. } => {
            let asked = LabelAsked::of(edit).ok_or(ApplyError::NoObject(object))?;
            apply_label(document, object, &asked, cancel)
        }
        _ => Err(ApplyError::NoObject(
            edit.object().unwrap_or(ObjectId::UNSET),
        )),
    }
}

/// Carries `edit` out on `document`, which an error leaves as it was; `cancel` stops one that
/// takes long enough to be given up on.
pub fn apply(
    document: &mut Document,
    edit: &Edit,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    match *edit {
        Edit::Cut { .. }
        | Edit::Split { .. }
        | Edit::PartOff { .. }
        | Edit::Combine { .. }
        | Edit::Label { .. } => apply_modelling(document, edit, cancel),
        Edit::Repair { .. }
        | Edit::Mend { .. }
        | Edit::Relief { .. }
        | Edit::Hollow { .. }
        | Edit::Drain { .. }
        | Edit::Channel { .. }
        | Edit::Plugs { .. }
        | Edit::Supports { .. }
        | Edit::MergeSupports { .. } => apply_to_the_surface(document, edit, cancel),
        Edit::Arrange {
            low_mm,
            plate_mm,
            clearance_mm,
        } => apply_arrange(document, (low_mm, plate_mm), clearance_mm),
        Edit::Orient { object, .. } => {
            let asked = OrientAsked::of(edit).ok_or(ApplyError::NoObject(object))?;
            apply_orient(document, object, &asked, cancel)
        }
        Edit::Move { object, .. }
        | Edit::Rotate { object, .. }
        | Edit::Scale { object, .. }
        | Edit::Mirror { object, .. } => apply_placement(document, object, edit),
        Edit::Delete { object } => delete(document, object),
        Edit::Duplicate { object, offset_mm } => duplicate(document, object, offset_mm),
    }
}

/// Moves, turns, sizes or mirrors object `id` as `edit` says; any other edit leaves it be.
fn apply_placement(
    document: &mut Document,
    id: ObjectId,
    edit: &Edit,
) -> Result<Applied, ApplyError> {
    place(document, id, |transform, mesh| match *edit {
        Edit::Move { by_mm, .. } => Ok(moved(transform, by_mm)),
        Edit::Rotate { degrees, .. } => Ok(turned_to(transform, mesh, degrees)),
        Edit::Scale { factors, .. } => {
            sized_to(transform, mesh, factors).ok_or(ApplyError::BadScale(id))
        }
        Edit::Mirror { axis, .. } => Ok(mirrored(transform, mesh, axis)),
        _ => Ok(transform),
    })
}

/// Sets the transform of `id` to what `to` makes of it and the object's own mesh.
fn place(
    document: &mut Document,
    id: ObjectId,
    to: impl FnOnce(Transform, &Mesh) -> Result<Transform, ApplyError>,
) -> Result<Applied, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let transform = to(state.transform, &meshes.source)?;
    stand_at(document, id, transform);
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report: Report::Placed,
    })
}

fn apply_cut(
    document: &mut Document,
    id: ObjectId,
    asked: &CutAsked,
) -> Result<Applied, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let hollow = hollow_of(&state.hollow, meshes);
    let model = CutModel {
        printed: meshes.printed().clone(),
        cuts: hollow.cut_bodies().cloned(),
        solid: meshes.shell.as_ref().map(|_| meshes.source.clone()),
        transform: state.transform,
    };
    let done = cut(&model, asked, id)?;
    let (placed, bare) = (done.joined.placed, done.joined.bare);
    let pieces = pieces(done.joined, asked);
    let mended = pieces.iter().filter(|piece| piece.mended).count();
    let added = document.replace(id, pieces);
    Ok(Applied {
        removed: vec![id],
        added,
        report: Report::Cut {
            open_loops: done.open_loops,
            placed,
            bare,
            unjoined: done.unjoined.map(|error| error.to_string()),
            mended,
        },
    })
}

#[cfg(test)]
mod tests;

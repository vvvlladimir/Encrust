//! Plugs laid beside an object for its drain holes (ADR 0313, 0321).

use std::sync::Arc;

use core_edit::ObjectId;
use core_engine::project::hollow_of;
use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec2, transform_mesh};
use core_plate::{ArrangeSettings, Rect, nearest_free};
use core_volume::{
    Cancel, DrainHole, ModelRead, PLUG_SPACING_MM, Plugs, VolumeError, laid_beside, plug_wall_mm,
    plugs,
};

use crate::apply::{Applied, Report};
use crate::arrange::{Standing, standing_rect};
use crate::document::Document;
use crate::error::ApplyError;
use crate::piece::{Piece, Role};
use crate::supports::standing_meshes;

/// A plug for each of `holes`, as they are cut, out of the solid `mesh`: `wall_mm` long where
/// there is a wall, `gap_mm` narrower a side.
pub(crate) fn plugs_cut(
    mesh: &Mesh,
    holes: &[DrainHole],
    (wall_mm, gap_mm): (Option<Scalar>, Scalar),
    cancel: Cancel<'_>,
) -> Result<Plugs, VolumeError> {
    let read = ModelRead::new(Arc::new(mesh.clone()));
    plugs(
        &mut None,
        &read,
        &Bvh::build(mesh),
        holes,
        wall_mm,
        gap_mm,
        cancel,
    )
}

/// `plugs`, plate mm, each moved to the nearest spot clear of `taken` and of one another in the
/// area `size_mm` across from `low_mm`, and how many fit nowhere, left where they were.
pub fn spaced_plugs(
    plugs: impl IntoIterator<Item = (usize, Mesh)>,
    taken: &[Rect],
    (low_mm, size_mm): (Vec2, Vec2),
) -> (Vec<(usize, Mesh)>, usize) {
    let settings = ArrangeSettings {
        clearance_mm: PLUG_SPACING_MM,
        ..ArrangeSettings::default()
    };
    let mut taken: Vec<Rect> = taken
        .iter()
        .map(|rect| Rect {
            min: rect.min - low_mm,
            max: rect.max - low_mm,
        })
        .collect();
    let mut crowded = 0;
    let mut spaced = Vec::new();
    for (number, plug) in plugs {
        let Some(bounds) = plug.aabb() else {
            continue;
        };
        let (min, max) = (
            bounds.mins.truncate() - low_mm,
            bounds.maxs.truncate() - low_mm,
        );
        let toward = (min + max) / 2.0;
        let at = nearest_free(max - min, &taken, toward, size_mm, &settings).unwrap_or_else(|| {
            crowded += 1;
            toward
        });
        let shift = at - toward;
        taken.push(Rect {
            min: min + shift,
            max: max + shift,
        });
        let moved = transform_mesh(&plug, Transform::from_translation(shift.extend(0.0)));
        spaced.push((number, moved));
    }
    (spaced, crowded)
}

/// What each printable object of the active plate covers with its supports and raft.
fn footprints(document: &Document) -> Vec<Rect> {
    let mut covered = Vec::new();
    for id in document.printable() {
        let Some((state, meshes)) = document.object(id) else {
            continue;
        };
        let supports = standing_meshes(document, id);
        let standing = Standing {
            mesh: meshes.printed(),
            transform: state.transform,
            supports: supports.iter().map(|mesh| &**mesh).collect(),
            moved: Transform::default(),
        };
        covered.extend(standing_rect(&standing));
    }
    covered
}

/// The plugs of object `id`'s holes, `gap_mm` narrower a side, each numbered after its hole and
/// laid in a row past the object, and how many came out closed.
fn numbered_plugs(
    document: &Document,
    id: ObjectId,
    gap_mm: Scalar,
    cancel: Cancel<'_>,
) -> Result<(Vec<(usize, Mesh)>, usize), ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let transform = state.transform;
    let hollow = hollow_of(&state.hollow, meshes);
    let least = transform.least_scale();
    let wall_mm =
        (hollow.asked()).and_then(|asked| plug_wall_mm(asked.mode, asked.thickness_mm / least));
    let holes = hollow.cut().0;
    let made = plugs_cut(&meshes.source, holes, (wall_mm, gap_mm / least), cancel)
        .map_err(|error| ApplyError::of_volume(id, error))?;
    let beside =
        (transform_mesh(meshes.printed(), transform).aabb()).ok_or(ApplyError::NoSurface(id))?;
    // `laid_beside` leaves out an empty plug, so each is numbered after its own hole.
    let numbers = (1..)
        .zip(&made.meshes)
        .filter(|(_, plug)| !plug.is_empty())
        .map(|(number, _)| number);
    let laid = laid_beside(&made.meshes, holes, transform, beside);
    Ok((numbers.zip(laid).collect(), made.closed))
}

/// Lays a plug `gap_mm` narrower a side than each hole of object `id` in place of those laid
/// before: in a row past it, or at the nearest clear spot of `area_mm` (corner, size).
pub(crate) fn apply_plugs(
    document: &mut Document,
    id: ObjectId,
    gap_mm: Scalar,
    area_mm: Option<([Scalar; 2], [Scalar; 2])>,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    let (numbered, closed) = numbered_plugs(document, id, gap_mm, cancel)?;
    let (removed, _) = document
        .lay_plugs(id, Vec::new())
        .ok_or(ApplyError::NoObject(id))?;
    let (numbered, crowded) = match area_mm {
        Some((low_mm, size_mm)) => {
            let area = (Vec2::from_array(low_mm), Vec2::from_array(size_mm));
            spaced_plugs(numbered, &footprints(document), area)
        }
        None => (numbered, 0),
    };
    let pieces: Vec<Piece> = numbered
        .into_iter()
        .map(|(number, mesh)| Piece::sound(Role::Plug(number), mesh))
        .collect();
    let plugs = pieces.len();
    let (_, added) = document
        .lay_plugs(id, pieces)
        .ok_or(ApplyError::NoObject(id))?;
    Ok(Applied {
        removed,
        added,
        report: Report::Plugged {
            plugs,
            closed,
            crowded,
        },
    })
}

//! A model cut across a plane into the halves kept, joined as asked (`docs/design/cutting.md`).

use std::sync::Arc;

use core_edit::{Axis, Keep, ObjectId};
use core_geometry::{Mesh, Plane, Scalar, Transform, Vec3, transform_mesh};
use core_joint::{CutJoined, Joined, JointKind, Joints, cut_and_join};
use core_volume::drilled;

use crate::error::ApplyError;
use crate::piece::{Piece, Role};

/// Space left between a cut's lower half and the dowels laid out beside it, and between one
/// dowel and the next, millimetres.
const DOWEL_GAP_MM: Scalar = 3.0;

/// One model to cut, in its own space and where it stands on the plate.
#[derive(Debug, Clone)]
pub struct CutModel {
    /// What prints: the model or its hollowed shell, its holes taken out where they have been.
    pub printed: Arc<Mesh>,
    /// The holes and channels still to be taken out of `printed` before it is cut.
    pub cuts: Option<Arc<Mesh>>,
    /// The model's outside, where what prints is its shell.
    pub solid: Option<Arc<Mesh>>,
    pub transform: Transform,
}

/// How a cut asked for is set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutAsked {
    pub axis: Axis,
    /// Where the plane crosses `axis`, plate millimetres.
    pub at_mm: Scalar,
    pub keep: Keep,
    pub joints: Joints,
    pub lay_flat: bool,
}

impl CutAsked {
    /// The plane in plate coordinates, looking up its axis.
    pub fn plane(&self) -> Plane {
        let normal = axis_normal(self.axis);
        Plane {
            point: normal * self.at_mm,
            normal,
        }
    }
}

/// The plate's unit direction along `axis`.
pub fn axis_normal(axis: Axis) -> Vec3 {
    match axis {
        Axis::X => Vec3::X,
        Axis::Y => Vec3::Y,
        Axis::Z => Vec3::Z,
    }
}

/// `model` cut by `asked`'s plane, its halves joined where both are kept. `id` names it in
/// the error: a plane that misses it, or holes that cannot be taken out of it exactly.
pub fn cut(model: &CutModel, asked: &CutAsked, id: ObjectId) -> Result<CutJoined, ApplyError> {
    // The halves are new models with no holes of their own, so they are cut from the drilled one.
    let printed = match &model.cuts {
        Some(cuts) => drilled(&model.printed, cuts).map_err(|_| ApplyError::Undrilled(id))?,
        None => (*model.printed).clone(),
    };
    let printed = transform_mesh(&printed, model.transform);
    let solid = (model.solid.as_ref()).map(|solid| transform_mesh(solid, model.transform));
    let joints = (asked.keep == Keep::Both).then_some(&asked.joints);
    cut_and_join(&printed, solid.as_ref(), asked.plane(), joints).ok_or(ApplyError::Missed(id))
}

/// The halves `asked` keeps, each laid on its cut face where asked unless it is a lower half
/// pins or a key stand on, and the loose dowels in a row beside the lower half.
pub fn pieces(joined: Joined, asked: &CutAsked) -> Vec<Piece> {
    let up = asked.plane().normal;
    let kind = asked.joints.kind.filter(|_| asked.keep == Keep::Both);
    // Pins and a tongue stand on the lower half's cut, which it then cannot lie on.
    let raised = matches!(kind, Some(JointKind::Pins | JointKind::Key)) && joined.placed > 0;
    let halves = match asked.keep {
        Keep::Both => vec![
            (Role::Lower, joined.below, up),
            (Role::Upper, joined.above, -up),
        ],
        Keep::Below => vec![(Role::Kept, joined.below, up)],
        Keep::Above => vec![(Role::Kept, joined.above, -up)],
    };
    let mut pieces: Vec<Piece> = halves
        .into_iter()
        .map(|(role, mesh, cut_face)| {
            let piece = Piece::sound(role, mesh);
            let is_lower = cut_face == up;
            if asked.lay_flat && !(is_lower && raised) {
                piece.laid_on(cut_face)
            } else {
                piece
            }
        })
        .collect();
    let lower = pieces.iter().find(|piece| piece.role == Role::Lower);
    if let Some(bounds) =
        lower.and_then(|piece| transform_mesh(&piece.mesh, piece.transform).aabb())
    {
        let mut x = bounds.maxs.x + DOWEL_GAP_MM;
        let dowels = joined.loose.into_iter().enumerate().map(|(index, dowel)| {
            let radius = dowel.radius_mm;
            let offset = Vec3::new(x + radius, bounds.mins.y + radius, 0.0);
            x += 2.0 * radius + DOWEL_GAP_MM;
            let placed = transform_mesh(&dowel.mesh, Transform::from_translation(offset));
            Piece::sound(Role::Dowel(index + 1), placed)
        });
        pieces.extend(dowels.collect::<Vec<_>>());
    }
    pieces
}

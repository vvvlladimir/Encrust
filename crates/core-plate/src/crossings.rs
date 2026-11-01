//! Which models on the plate meet another: a face of one, or of its supports, crosses a face
//! of the other, or one stands inside it. See `docs/decisions/0390-models-meeting-on-the-plate.md`.

use std::sync::OnceLock;

use core_geometry::{
    Aabb, Buckets, Bvh, FastMap, FastSet, Mesh, PlacedMesh, Scalar, Transform, Winding,
    placed_aabb, placed_faces_cross,
};
use rayon::prelude::*;

/// How wide a bucket of the search for neighbours is, millimetres: about a small model, so
/// one meets few buckets and a bucket holds few bodies.
const BUCKET_MM: Scalar = 10.0;

/// What a body on the plate is: a model, or the supports and raft standing under one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Model,
    Supports,
}

/// One body standing on the plate, as [`crossings`] reads it.
#[derive(Debug, Clone, Copy)]
pub struct Body<'a> {
    /// Which model it is or stands under; the bodies of one model never meet each other.
    pub owner: usize,
    pub kind: BodyKind,
    pub mesh: &'a Mesh,
    /// The hierarchy of `mesh`'s faces where the caller holds one; built when asked otherwise.
    pub bvh: Option<&'a Bvh>,
    pub transform: Transform,
}

/// The pairs of models that meet, lower owner first, ascending. Supports meeting supports
/// are not a meeting: they print as one and come off together.
pub fn crossings(bodies: &[Body<'_>]) -> Vec<[usize; 2]> {
    let bounds: Vec<Option<Aabb>> = bodies
        .par_iter()
        .map(|body| placed_aabb(body.mesh, body.transform))
        .collect();
    let lazy = Lazy::new(bodies.len());
    let mut met: Vec<[usize; 2]> = candidates(bodies, &bounds)
        .into_par_iter()
        .filter(|(_, pairs)| {
            pairs
                .iter()
                .any(|&[one, other]| meet(bodies, &bounds, &lazy, one, other))
        })
        .map(|(owners, _)| owners)
        .collect();
    met.sort_unstable();
    met
}

/// The pairs of bodies whose boxes meet, grouped by the two models they belong to.
fn candidates(
    bodies: &[Body<'_>],
    bounds: &[Option<Aabb>],
) -> FastMap<[usize; 2], Vec<[usize; 2]>> {
    let mut buckets = Buckets::new(BUCKET_MM);
    for (index, found) in bounds.iter().enumerate() {
        if let Some(found) = found {
            buckets.insert_over(found.mins.truncate(), found.maxs.truncate(), &index);
        }
    }
    let mut seen = FastSet::default();
    let mut grouped: FastMap<[usize; 2], Vec<[usize; 2]>> = FastMap::default();
    for (one, found) in bounds.iter().enumerate() {
        let Some(found) = found else { continue };
        for &other in buckets.within(found.mins.truncate(), found.maxs.truncate()) {
            let (a, b) = (&bodies[one], &bodies[other]);
            let apart = a.owner == b.owner
                || (a.kind == BodyKind::Supports && b.kind == BodyKind::Supports)
                || !bounds[other].is_some_and(|box_b| boxes_meet(found, &box_b));
            if apart || other <= one || !seen.insert([one, other]) {
                continue;
            }
            let owners = [a.owner.min(b.owner), a.owner.max(b.owner)];
            grouped.entry(owners).or_default().push([one, other]);
        }
    }
    grouped
}

fn boxes_meet(one: &Aabb, other: &Aabb) -> bool {
    one.mins.cmple(other.maxs).all() && other.mins.cmple(one.maxs).all()
}

/// The hierarchies each body is asked through, built the first time a pair needs them.
struct Lazy {
    bvhs: Vec<OnceLock<Bvh>>,
    windings: Vec<OnceLock<Winding>>,
}

impl Lazy {
    fn new(count: usize) -> Self {
        Self {
            bvhs: (0..count).map(|_| OnceLock::new()).collect(),
            windings: (0..count).map(|_| OnceLock::new()).collect(),
        }
    }
}

/// Whether two bodies meet: their faces cross, or one stands wholly inside the other.
fn meet(
    bodies: &[Body<'_>],
    bounds: &[Option<Aabb>],
    lazy: &Lazy,
    one: usize,
    other: usize,
) -> bool {
    let (Some(box_one), Some(box_other)) = (bounds[one], bounds[other]) else {
        return false;
    };
    let placed = |index: usize, bounds_mm: Aabb| {
        let body = &bodies[index];
        let bvh = body
            .bvh
            .unwrap_or_else(|| lazy.bvhs[index].get_or_init(|| Bvh::build(body.mesh)));
        PlacedMesh {
            mesh: body.mesh,
            bvh,
            transform: body.transform,
            bounds_mm,
        }
    };
    placed_faces_cross(&placed(one, box_one), &placed(other, box_other))
        || inside(bodies, lazy, (one, &box_one), (other, &box_other))
        || inside(bodies, lazy, (other, &box_other), (one, &box_one))
}

/// Whether `inner`, crossing none of `outer`'s faces, stands inside it: then any one of its
/// vertices does, and only a box inside the other's can.
fn inside(bodies: &[Body<'_>], lazy: &Lazy, inner: (usize, &Aabb), outer: (usize, &Aabb)) -> bool {
    let ((inner, inner_box), (outer, outer_box)) = (inner, outer);
    let contained =
        outer_box.mins.cmple(inner_box.mins).all() && inner_box.maxs.cmple(outer_box.maxs).all();
    let (body, around) = (&bodies[inner], &bodies[outer]);
    let (true, Some(vertex), Some(into_outer)) = (
        contained,
        body.mesh.vertices.first(),
        around.transform.inverse_matrix(),
    ) else {
        return false;
    };
    let at = into_outer.transform_point3(body.transform.to_matrix().transform_point3(*vertex));
    let winding = lazy.windings[outer].get_or_init(|| Winding::build(around.mesh));
    winding.is_inside(around.mesh, at)
}

#[cfg(test)]
mod tests;

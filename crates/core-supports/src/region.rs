use std::ops::Range;
use std::sync::Arc;

use core_geometry::{Bvh, Mat4, Mesh, Region, Scalar, Transform, Vec3};

/// How far off the blocked patch a point on the model may lie and still be on it,
/// millimetres: a hundredth, well over what single precision loses across a plate.
const ON_PATCH_MM: Scalar = 0.01;

/// A region the user has forbidden supports, as the question every placement asks: is a face
/// blocked, and is a plate point over blocked surface (ADR 0093).
#[derive(Debug)]
pub struct Blocked {
    patch: Option<Bvh>,
    side: Option<WallSide>,
}

/// One side of a hollow model's wall, which an automatic run keeps its contacts to: the
/// face of the printed surface nearest a point says which side it is on. See ADR 0332.
#[derive(Debug, Clone)]
pub struct WallSide {
    /// The printed surface in the model's own space.
    pub bvh: Arc<Bvh>,
    /// The faces of it that bound the cavity, or stand in it.
    pub inner: Range<usize>,
    /// From plate coordinates into the model's own.
    pub into_model: Mat4,
    /// Whether the run holds up the cavity rather than everything outside it.
    pub inside: bool,
}

impl WallSide {
    /// Whether `point`, in plate coordinates, is on the side kept.
    pub fn keeps(&self, point: Vec3) -> bool {
        let local = self.into_model.transform_point3(point);
        let inner = self
            .bvh
            .closest(local)
            .is_some_and(|closest| self.inner.contains(&closest.face));
        inner == self.inside
    }
}

impl Blocked {
    /// Builds the question for `region` marked on `model` standing at `transform`, or
    /// `None` when nothing is marked and every placement is free.
    pub fn new(model: &Mesh, region: &Region, transform: Transform) -> Option<Self> {
        let patch = region.submesh(model, transform, 0.0)?;
        Some(Self {
            patch: Some(Bvh::build(&patch)),
            side: None,
        })
    }

    /// `blocked`, also keeping automatic contacts off the far side of `side`'s wall, or
    /// `None` when there is neither.
    pub fn keeping(blocked: Option<Self>, side: Option<WallSide>) -> Option<Self> {
        match (blocked, side) {
            (blocked, None) => blocked,
            (blocked, Some(side)) => Some(Self {
                patch: blocked.and_then(|blocked| blocked.patch),
                side: Some(side),
            }),
        }
    }

    /// Whether `point`, in plate coordinates, lies on the blocked patch.
    pub fn holds(&self, point: Vec3) -> bool {
        self.near_patch(point, ON_PATCH_MM)
    }

    /// Whether the blocked patch comes within `radius_mm` of `point`, plate coordinates, or the
    /// point is on the side of the wall a run keeps off: keeps automatic contacts out.
    pub fn covers(&self, point: Vec3, radius_mm: Scalar) -> bool {
        self.near_patch(point, radius_mm)
            || self.side.as_ref().is_some_and(|side| !side.keeps(point))
    }

    fn near_patch(&self, point: Vec3, radius_mm: Scalar) -> bool {
        self.patch.as_ref().is_some_and(|patch| {
            patch
                .closest(point)
                .is_some_and(|closest| closest.distance <= radius_mm)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::box_mesh;

    /// The box every test paints on: 10 mm on a side, standing on the plate.
    fn cube() -> Mesh {
        box_mesh(Vec3::ZERO, Vec3::splat(10.0))
    }

    #[test]
    fn nothing_marked_blocks_nothing() {
        assert!(Blocked::new(&cube(), &Region::default(), Transform::default()).is_none());
    }

    #[test]
    fn a_blocked_patch_answers_for_its_faces_and_for_the_plate_over_it() {
        let model = cube();
        let bvh = Bvh::build(&model);
        let mut region = Region::default();
        region.brush(
            (&model, &bvh, Transform::default()),
            Vec3::new(5.0, 5.0, 11.0),
            2.0,
            true,
        );
        let face = region.faces().next().expect("the top was painted");

        let blocked =
            Blocked::new(&model, &region, Transform::default()).expect("a patch was painted");
        let painted = model.triangle(face).expect("the face was painted");
        assert!(blocked.holds((painted.a + painted.b + painted.c) / 3.0));
        assert!(
            !blocked.holds(Vec3::new(5.0, 5.0, 0.0)),
            "the cube's underside was not painted"
        );
        assert!(
            blocked.covers(Vec3::new(5.0, 5.0, 10.2), 0.5),
            "a contact a fifth of a millimetre under the painted top is covered"
        );
        assert!(
            !blocked.covers(Vec3::new(5.0, 5.0, 2.0), 0.5),
            "a contact halfway down the side is nowhere near the painted top"
        );
    }

    /// A 10 mm cube with a cavity 1 mm in from every face, the cavity's faces last.
    fn hollow_cube() -> (Mesh, Range<usize>) {
        let outer = cube();
        let inner = box_mesh(Vec3::splat(1.0), Vec3::splat(9.0));
        let offset = outer.vertices.len() as u32;
        let vertices = [outer.vertices.clone(), inner.vertices].concat();
        let faces: Vec<[u32; 3]> = outer
            .faces
            .iter()
            .copied()
            .chain(
                inner
                    .faces
                    .iter()
                    .map(|face| face.map(|vertex| vertex + offset)),
            )
            .collect();
        let cavity = outer.faces.len()..faces.len();
        (Mesh::new(vertices, faces), cavity)
    }

    #[test]
    fn a_side_of_the_wall_keeps_contacts_under_its_own_surface_and_refuses_the_other() {
        let (shell, cavity) = hollow_cube();
        let side = |inside| WallSide {
            bvh: Arc::new(Bvh::build(&shell)),
            inner: cavity.clone(),
            into_model: Mat4::IDENTITY,
            inside,
        };
        // A layer under the outer floor, and a layer under the cavity's ceiling.
        let under_floor = Vec3::new(5.0, 5.0, -0.05);
        let under_ceiling = Vec3::new(5.0, 5.0, 8.95);

        let outside = Blocked::keeping(None, Some(side(false))).expect("a side is kept");
        assert!(!outside.covers(under_floor, 0.5), "the outside is held");
        assert!(
            outside.covers(under_ceiling, 0.0),
            "the cavity's ceiling 1 mm through the wall is not, at any radius"
        );
        let inside = Blocked::keeping(None, Some(side(true))).expect("a side is kept");
        assert!(!inside.covers(under_ceiling, 0.5));
        assert!(inside.covers(under_floor, 0.0));
        assert!(
            !inside.holds(under_floor),
            "a hand placing on the floor is not stopped by a run's side"
        );
    }
}

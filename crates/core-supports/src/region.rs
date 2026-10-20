use core_geometry::{Bvh, Mesh, Region, Scalar, Transform, Vec3};

/// A region the user has forbidden supports, as the question every placement asks it.
///
/// Both halves of that question are answered here: a face is blocked, so no tip and no
/// foot may be put on it, and a point on the plate is over blocked surface, so automatic
/// placement puts nothing there. See `docs/decisions/0093`.
#[derive(Debug)]
pub struct Blocked {
    region: Region,
    bvh: Bvh,
}

impl Blocked {
    /// Builds the question for `region` marked on `model` standing at `transform`, or
    /// `None` when nothing is marked and every placement is free.
    pub fn new(model: &Mesh, region: &Region, transform: Transform) -> Option<Self> {
        let patch = region.submesh(model, transform, 0.0)?;
        Some(Self {
            region: region.clone(),
            bvh: Bvh::build(&patch),
        })
    }

    /// Whether `face` of the model is inside the blocked patch.
    pub fn contains_face(&self, face: usize) -> bool {
        self.region.contains(face)
    }

    /// Whether the blocked patch comes within `radius_mm` of `point`, in plate
    /// coordinates: what keeps an automatic contact out of a painted area.
    pub fn covers(&self, point: Vec3, radius_mm: Scalar) -> bool {
        self.bvh
            .closest(point)
            .is_some_and(|closest| closest.distance <= radius_mm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::box_mesh;

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
        assert!(blocked.contains_face(face));
        assert!(!blocked.contains_face(model.faces.len()));
        assert!(
            blocked.covers(Vec3::new(5.0, 5.0, 10.2), 0.5),
            "a contact a fifth of a millimetre under the painted top is covered"
        );
        assert!(
            !blocked.covers(Vec3::new(5.0, 5.0, 2.0), 0.5),
            "a contact halfway down the side is nowhere near the painted top"
        );
    }
}

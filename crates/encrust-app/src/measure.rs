use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3};

/// How near a corner a click has to land, in millimetres of the plate, before it is read
/// as that corner. A model is measured across its features, and a click that is half a
/// millimetre off a corner meant the corner.
const SNAP_MM: Scalar = 0.5;

/// The two points the Measure tool is holding, in plate coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Measure {
    from: Option<Vec3>,
    to: Option<Vec3>,
}

impl Measure {
    /// Takes the next point: the first click starts a span, the second closes it, and the
    /// third starts a new one.
    pub fn pick(&mut self, point: Vec3) {
        match (self.from, self.to) {
            (Some(_), None) => self.to = Some(point),
            _ => {
                self.from = Some(point);
                self.to = None;
            }
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn start(&self) -> Option<Vec3> {
        self.from
    }

    /// Both ends, once both have been picked.
    pub fn span(&self) -> Option<(Vec3, Vec3)> {
        self.from.zip(self.to)
    }

    /// How far apart the two ends are, in millimetres.
    pub fn distance_mm(&self) -> Option<Scalar> {
        self.span().map(|(from, to)| (to - from).length())
    }

    /// The span broken into its three axes, in millimetres.
    pub fn delta_mm(&self) -> Option<Vec3> {
        self.span().map(|(from, to)| to - from)
    }
}

/// The corner of the model nearest `point`, or `point` itself when the click landed in
/// the middle of a face.
///
/// `point` and the result are both in plate coordinates; the search runs in the model's
/// own space, which is the space the hierarchy was built in.
pub fn snap(mesh: &Mesh, bvh: &Bvh, transform: Transform, point: Vec3) -> Vec3 {
    let matrix = transform.to_matrix();
    let Some(inverse) = matrix.inverse().is_finite().then(|| matrix.inverse()) else {
        return point;
    };

    let local = inverse.transform_point3(point);
    let Some(near) = bvh.closest(mesh, local) else {
        return point;
    };
    let Some(face) = mesh.faces.get(near.face) else {
        return point;
    };

    face.iter()
        .filter_map(|index| mesh.vertices.get(*index as usize))
        .map(|vertex| matrix.transform_point3(*vertex))
        .map(|corner| (corner, (corner - point).length()))
        .filter(|(_, distance)| *distance <= SNAP_MM)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(point, |(corner, _)| corner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Axis-aligned cube spanning 0..1 on every axis, twelve triangles.
    fn unit_cube() -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Mesh::new(vertices, faces)
    }

    #[test]
    fn two_picks_make_a_span_and_the_third_starts_again() {
        let mut measure = Measure::default();
        assert!(measure.span().is_none());

        measure.pick(Vec3::ZERO);
        assert!(measure.span().is_none(), "one end is not a measurement");

        measure.pick(Vec3::new(3.0, 4.0, 0.0));
        assert_eq!(measure.distance_mm(), Some(5.0), "a 3-4-5 triangle");

        measure.pick(Vec3::new(1.0, 1.0, 1.0));
        assert!(measure.span().is_none());
        assert_eq!(measure.start(), Some(Vec3::new(1.0, 1.0, 1.0)));
    }

    #[test]
    fn the_span_is_reported_axis_by_axis() {
        let mut measure = Measure::default();
        measure.pick(Vec3::new(1.0, 2.0, 3.0));
        measure.pick(Vec3::new(4.0, 2.0, 8.0));
        assert_eq!(measure.delta_mm(), Some(Vec3::new(3.0, 0.0, 5.0)));
    }

    #[test]
    fn clearing_forgets_both_ends() {
        let mut measure = Measure::default();
        measure.pick(Vec3::ZERO);
        measure.pick(Vec3::X);
        measure.clear();
        assert_eq!(measure, Measure::default());
    }

    #[test]
    fn a_click_near_a_corner_measures_the_corner() {
        let mesh = Arc::new(unit_cube());
        let bvh = Bvh::build(&mesh);
        let near_corner = Vec3::new(0.98, 0.99, 1.0);

        let snapped = snap(&mesh, &bvh, Transform::default(), near_corner);
        assert!(
            snapped.abs_diff_eq(Vec3::new(1.0, 1.0, 1.0), 1e-5),
            "the corner of the unit cube, got {snapped}"
        );
    }

    #[test]
    fn a_click_in_the_middle_of_a_face_stays_where_it_landed() {
        let mesh = Arc::new(unit_cube());
        let bvh = Bvh::build(&mesh);
        let middle = Vec3::new(0.5, 0.5, 1.0);

        let snapped = snap(&mesh, &bvh, Transform::default(), middle);
        assert!(
            snapped.abs_diff_eq(middle, 1e-6),
            "no corner is within reach"
        );
    }

    #[test]
    fn a_corner_is_measured_where_the_model_stands() {
        let mesh = Arc::new(unit_cube());
        let bvh = Bvh::build(&mesh);
        let placed = Transform {
            translation: Vec3::new(10.0, 0.0, 0.0),
            scale: Vec3::splat(2.0),
            ..Transform::default()
        };

        let snapped = snap(&mesh, &bvh, placed, Vec3::new(11.9, 1.9, 2.0));
        assert!(
            snapped.abs_diff_eq(Vec3::new(12.0, 2.0, 2.0), 1e-5),
            "the corner in plate coordinates, got {snapped}"
        );
    }

    #[test]
    fn a_flattened_model_cannot_be_snapped_to() {
        let mesh = Arc::new(unit_cube());
        let bvh = Bvh::build(&mesh);
        let flat = Transform {
            scale: Vec3::new(1.0, 1.0, 0.0),
            ..Transform::default()
        };

        let point = Vec3::new(0.99, 0.99, 0.0);
        assert_eq!(snap(&mesh, &bvh, flat, point), point);
    }
}

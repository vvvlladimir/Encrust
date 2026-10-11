use std::sync::Arc;

use core_geometry::{Adjacency, Bvh, Mesh, Scalar, Transform, Vec3};

/// How near a corner a click has to land, in millimetres of the plate, before it is read
/// as that corner. A model is measured across its features, and a click that is half a
/// millimetre off a corner meant the corner.
const SNAP_MM: Scalar = 0.5;

/// What a picked point was caught by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Snapped {
    Corner,
    Edge,
    /// Where the click landed: in the middle of a face, or with Alt held.
    Free,
}

impl Snapped {
    pub fn label(self) -> &'static str {
        match self {
            Self::Corner => "on a corner",
            Self::Edge => "on an edge",
            Self::Free => "free",
        }
    }
}

/// How far the surface has to turn across an edge for the edge to be one a click snaps
/// to, degrees. Under this it is the seam between two triangles of one flat or gently
/// curved face, which nobody means to measure from.
const FEATURE_DEG: Scalar = 20.0;

/// The faces sharing each edge of the last mesh measured on. Built once per mesh, since a
/// mesh of a million faces takes a moment.
#[derive(Default)]
pub struct EdgeCache {
    adjacency: Option<(usize, Arc<Adjacency>)>,
}

impl EdgeCache {
    pub fn of(&mut self, mesh: &Arc<Mesh>) -> Arc<Adjacency> {
        let key = Arc::as_ptr(mesh) as usize;
        match &self.adjacency {
            Some((built_for, adjacency)) if *built_for == key => Arc::clone(adjacency),
            _ => {
                let adjacency = Arc::new(Adjacency::of(mesh));
                self.adjacency = Some((key, Arc::clone(&adjacency)));
                adjacency
            }
        }
    }
}

/// The two points the Measure tool is holding, in plate coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Measure {
    from: Option<Vec3>,
    to: Option<Vec3>,
    /// What caught the last point picked.
    snapped: Option<Snapped>,
}

impl Measure {
    /// Takes the next point: the first click starts a span, the second closes it, and the
    /// third starts a new one.
    pub fn pick(&mut self, point: Vec3, snapped: Snapped) {
        self.snapped = Some(snapped);
        match (self.from, self.to) {
            (Some(_), None) => self.to = Some(point),
            _ => {
                self.from = Some(point);
                self.to = None;
            }
        }
    }

    pub fn snapped(&self) -> Option<Snapped> {
        self.snapped
    }

    pub fn is_empty(&self) -> bool {
        self.from.is_none()
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

/// The corner of the model nearest `point`, else the nearest point on an edge where the
/// surface turns, within `SNAP_MM` of it; else `point` itself, where the click landed in
/// the middle of a face.
///
/// `point` and the result are both in plate coordinates; the search runs in the model's
/// own space, which is the space the hierarchy was built in.
pub fn snap(
    mesh: &Mesh,
    bvh: &Bvh,
    adjacency: &Adjacency,
    transform: Transform,
    point: Vec3,
) -> (Vec3, Snapped) {
    let free = (point, Snapped::Free);
    let matrix = transform.to_matrix();
    let Some(inverse) = matrix.inverse().is_finite().then(|| matrix.inverse()) else {
        return free;
    };

    let local = inverse.transform_point3(point);
    let Some(near) = bvh.closest(mesh, local) else {
        return free;
    };
    let Some(face) = mesh.faces.get(near.face) else {
        return free;
    };
    let world = |index: u32| {
        mesh.vertices
            .get(index as usize)
            .map(|vertex| matrix.transform_point3(*vertex))
    };
    let nearest = |candidates: &mut dyn Iterator<Item = Vec3>| {
        candidates
            .map(|candidate| (candidate, (candidate - point).length()))
            .filter(|(_, distance)| *distance <= SNAP_MM)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(candidate, _)| candidate)
    };
    if let Some(corner) = nearest(&mut face.iter().filter_map(|index| world(*index))) {
        return (corner, Snapped::Corner);
    }
    let mut edges = (0..3)
        .map(|side| (face[side], face[(side + 1) % 3]))
        .filter(|(a, b)| is_feature(mesh, adjacency, near.face, *a, *b))
        .filter_map(|(a, b)| Some(on_segment(point, world(a)?, world(b)?)));
    nearest(&mut edges).map_or(free, |edge| (edge, Snapped::Edge))
}

/// Whether the edge from `a` to `b` of `face` is where the surface turns: the face across
/// it leans past `FEATURE_DEG`, or there is none.
fn is_feature(mesh: &Mesh, adjacency: &Adjacency, face: usize, a: u32, b: u32) -> bool {
    let normal = |index: usize| {
        mesh.triangle(index)
            .map(|triangle| triangle.normal_unnormalized().normalize_or_zero())
    };
    let Some(own) = normal(face) else {
        return false;
    };
    let across = adjacency.neighbours(face).iter().find(|other| {
        mesh.faces
            .get(**other as usize)
            .is_some_and(|corners| corners.contains(&a) && corners.contains(&b))
    });
    across
        .and_then(|other| normal(*other as usize))
        .is_none_or(|theirs| own.dot(theirs) < FEATURE_DEG.to_radians().cos())
}

/// The point of the segment from `a` to `b` nearest `point`.
fn on_segment(point: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let along = b - a;
    let length_sq = along.length_squared();
    if length_sq <= Scalar::EPSILON {
        return a;
    }
    a + along * ((point - a).dot(along) / length_sq).clamp(0.0, 1.0)
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

        measure.pick(Vec3::ZERO, Snapped::Free);
        assert!(measure.span().is_none(), "one end is not a measurement");

        measure.pick(Vec3::new(3.0, 4.0, 0.0), Snapped::Free);
        assert_eq!(measure.distance_mm(), Some(5.0), "a 3-4-5 triangle");

        measure.pick(Vec3::new(1.0, 1.0, 1.0), Snapped::Free);
        assert!(measure.span().is_none());
        assert_eq!(measure.start(), Some(Vec3::new(1.0, 1.0, 1.0)));
    }

    #[test]
    fn the_span_is_reported_axis_by_axis() {
        let mut measure = Measure::default();
        measure.pick(Vec3::new(1.0, 2.0, 3.0), Snapped::Free);
        measure.pick(Vec3::new(4.0, 2.0, 8.0), Snapped::Free);
        assert_eq!(measure.delta_mm(), Some(Vec3::new(3.0, 0.0, 5.0)));
    }

    #[test]
    fn clearing_forgets_both_ends() {
        let mut measure = Measure::default();
        measure.pick(Vec3::ZERO, Snapped::Free);
        measure.pick(Vec3::X, Snapped::Free);
        measure.clear();
        assert_eq!(measure, Measure::default());
    }

    /// Snaps on the unit cube, placed by `transform`.
    fn snap_on_cube(transform: Transform, point: Vec3) -> (Vec3, Snapped) {
        let mesh = Arc::new(unit_cube());
        let bvh = Bvh::build(&mesh);
        snap(&mesh, &bvh, &Adjacency::of(&mesh), transform, point)
    }

    #[test]
    fn a_click_near_a_corner_measures_the_corner() {
        let near_corner = Vec3::new(0.98, 0.99, 1.0);

        let (snapped, by) = snap_on_cube(Transform::default(), near_corner);
        assert_eq!(by, Snapped::Corner);
        assert!(
            snapped.abs_diff_eq(Vec3::new(1.0, 1.0, 1.0), 1e-5),
            "the corner of the unit cube, got {snapped}"
        );
    }

    #[test]
    fn a_click_in_the_middle_of_a_face_stays_where_it_landed() {
        // A 10 mm cube, on the diagonal its top is split along, which is no edge of it.
        let middle = Vec3::new(5.0, 5.0, 10.0);
        let ten = Transform {
            scale: Vec3::splat(10.0),
            ..Transform::default()
        };

        let (snapped, by) = snap_on_cube(ten, middle);
        assert_eq!(by, Snapped::Free);
        assert!(
            snapped.abs_diff_eq(middle, 1e-6),
            "no corner is within reach"
        );
    }

    #[test]
    fn a_corner_is_measured_where_the_model_stands() {
        let placed = Transform {
            translation: Vec3::new(10.0, 0.0, 0.0),
            scale: Vec3::splat(2.0),
            ..Transform::default()
        };

        let (snapped, _) = snap_on_cube(placed, Vec3::new(11.9, 1.9, 2.0));
        assert!(
            snapped.abs_diff_eq(Vec3::new(12.0, 2.0, 2.0), 1e-5),
            "the corner in plate coordinates, got {snapped}"
        );
    }

    #[test]
    fn a_flattened_model_cannot_be_snapped_to() {
        let flat = Transform {
            scale: Vec3::new(1.0, 1.0, 0.0),
            ..Transform::default()
        };

        let point = Vec3::new(0.99, 0.99, 0.0);
        assert_eq!(snap_on_cube(flat, point), (point, Snapped::Free));
    }

    #[test]
    fn a_click_near_an_edge_measures_the_edge() {
        // 0.1 mm in from the top front edge of the cube, half way along it.
        let (snapped, by) = snap_on_cube(Transform::default(), Vec3::new(0.5, 0.1, 1.0));
        assert_eq!(by, Snapped::Edge);
        assert!(
            snapped.abs_diff_eq(Vec3::new(0.5, 0.0, 1.0), 1e-5),
            "the point on the edge straight across, got {snapped}"
        );
    }
}

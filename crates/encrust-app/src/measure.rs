use std::sync::Arc;

use core_geometry::{
    Adjacency, Bvh, FastSet, Mat4, Mesh, Scalar, Transform, Triangle, Vec3, point_triangle,
};
use core_supports::Region;

use crate::i18n::tr;
use crate::scene::{ObjectId, SceneObject};

/// The most triangles a snap looks through around the click: a reach that takes in more
/// is a model zoomed out so far that the triangle clicked is as good as any.
const MAX_AROUND: usize = 20_000;

/// How far a triangle may lean from the one clicked and still be part of the same plane,
/// degrees. A sculpted or scanned flat face wavers by a degree or two.
const PLANE_DEG: Scalar = 3.0;

/// Two planes leaning less than this apart are read as parallel, and the gap between
/// them is a thickness, degrees.
const PARALLEL_DEG: Scalar = 1.0;

/// What a picked point was caught by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Snapped {
    Corner,
    Edge,
    /// Where the click landed: in the middle of a face, or with Alt held.
    Free,
}

impl Snapped {
    pub fn label(self) -> String {
        match self {
            Self::Corner => tr!("measure-snapped-corner"),
            Self::Edge => tr!("measure-snapped-edge"),
            Self::Free => tr!("measure-snapped-free"),
        }
    }
}

/// What a click of the Measure tool is caught by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Snap {
    /// The nearest corner where the surface turns, else a corner of the triangle clicked.
    Corner,
    /// The nearest edge where the surface turns, else an edge of the triangle clicked.
    #[default]
    Edge,
    /// Where the click landed, always. Held Alt reads as this whatever is chosen.
    Free,
}

/// What the Measure tool measures between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Target {
    #[default]
    Points,
    /// Two flat faces: the gap across them and the angle between them.
    Planes,
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

/// A flat face of a model picked to measure from, placed where the model stands.
#[derive(Debug, Clone, PartialEq)]
pub struct Plane {
    pub id: ObjectId,
    mesh: usize,
    /// The triangles of the face, indices into the model's mesh.
    pub faces: Vec<usize>,
    /// Area-weighted centre, plate coordinates.
    pub centre: Vec3,
    /// Unit, plate coordinates, pointing out of the model.
    pub normal: Vec3,
}

impl Plane {
    /// Every triangle of `object` joined to `seed` within `PLANE_DEG`, as one plane.
    pub fn of(object: &SceneObject, adjacency: &Adjacency, seed: usize) -> Option<Self> {
        let mut region = Region::default();
        region.flood(&object.mesh, adjacency, seed, PLANE_DEG, true);
        let faces: Vec<usize> = region.faces().collect();
        let matrix = object.transform.to_matrix();
        let (mut normal, mut centre, mut area) = (Vec3::ZERO, Vec3::ZERO, 0.0);
        for triangle in faces.iter().filter_map(|face| object.mesh.triangle(*face)) {
            let [a, b, c] =
                [triangle.a, triangle.b, triangle.c].map(|v| matrix.transform_point3(v));
            let twice = (b - a).cross(c - a);
            normal += twice;
            centre += (a + b + c) / 3.0 * twice.length();
            area += twice.length();
        }
        Some(Self {
            id: object.id,
            mesh: Arc::as_ptr(&object.mesh) as usize,
            faces,
            centre: (area > Scalar::EPSILON).then(|| centre / area)?,
            normal: normal.try_normalize()?,
        })
    }

    /// Whether this was picked on `object` as it is now, so its faces still index its mesh.
    pub fn is_on(&self, object: &SceneObject) -> bool {
        self.id == object.id && self.mesh == Arc::as_ptr(&object.mesh) as usize
    }
}

/// How two planes stand to each other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneGap {
    /// Between the two planes, whichever way each faces, degrees.
    pub angle_deg: Scalar,
    /// Square across from the first plane to the second one's centre, millimetres: the
    /// thickness between them when they are parallel.
    pub gap_mm: Scalar,
    /// From one centre to the other, millimetres.
    pub centres_mm: Scalar,
    /// The line the view draws: square across the gap when the planes are parallel, from
    /// centre to centre when they are not, since square across a tilted plane lands
    /// wherever its slope carries it.
    pub from: Vec3,
    pub to: Vec3,
}

impl PlaneGap {
    /// Whether the planes are parallel, so the distance is the thickness between them.
    pub fn is_parallel(&self) -> bool {
        self.angle_deg <= PARALLEL_DEG
    }
}

/// What the Measure tool is set to, and the two points or planes it is holding, in plate
/// coordinates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Measure {
    pub snap: Snap,
    pub target: Target,
    /// Draws the span's run along X, Y and Z in the view beside its straight line.
    pub show_axes: bool,
    from: Option<Vec3>,
    to: Option<Vec3>,
    /// What caught the last point picked.
    snapped: Option<Snapped>,
    planes: [Option<Plane>; 2],
}

impl Measure {
    /// What a click is caught by now: Alt held measures freely whatever is chosen.
    pub fn snap_now(&self, alt: bool) -> Snap {
        if alt { Snap::Free } else { self.snap }
    }

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

    /// Takes the next plane, the way [`Measure::pick`] takes the next point.
    pub fn pick_plane(&mut self, plane: Plane) {
        match &self.planes {
            [Some(_), None] => self.planes[1] = Some(plane),
            _ => self.planes = [Some(plane), None],
        }
    }

    pub fn snapped(&self) -> Option<Snapped> {
        self.snapped
    }

    /// Whether nothing is picked for what the tool is measuring between.
    pub fn is_empty(&self) -> bool {
        match self.target {
            Target::Points => self.from.is_none(),
            Target::Planes => self.planes[0].is_none(),
        }
    }

    /// Forgets what was picked, and keeps how the tool is set.
    pub fn clear(&mut self) {
        *self = Self {
            snap: self.snap,
            target: self.target,
            show_axes: self.show_axes,
            ..Self::default()
        };
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

    /// The planes picked so far, first first.
    pub fn planes(&self) -> impl Iterator<Item = &Plane> {
        self.planes.iter().flatten()
    }

    /// How the two picked planes stand to each other, once both are picked.
    pub fn plane_gap(&self) -> Option<PlaneGap> {
        let [Some(first), Some(second)] = &self.planes else {
            return None;
        };
        let along = first.normal.dot(second.centre - first.centre);
        let cos = first.normal.dot(second.normal).abs().min(1.0);
        let angle_deg = cos.acos().to_degrees();
        let to = match angle_deg <= PARALLEL_DEG {
            true => second.centre - first.normal * along,
            false => first.centre,
        };
        Some(PlaneGap {
            angle_deg,
            gap_mm: along.abs(),
            centres_mm: (second.centre - first.centre).length(),
            from: second.centre,
            to,
        })
    }
}

/// Where a click at `point` is caught: the nearest corner, or the nearest point of an
/// edge, where the surface turns, as `to` asks. It is looked for within `reach_mm`, then
/// `WIDER` times as far; past that the click stays where it landed, since a seam between
/// two triangles of one smooth surface is no edge anybody means. `Snap::Free` keeps it too.
///
/// `point`, `reach_mm` and the result are in plate coordinates; the triangle clicked is
/// found in the model's own space, which is the space the hierarchy was built in.
pub fn snap(
    mesh: &Mesh,
    bvh: &Bvh,
    adjacency: &Adjacency,
    transform: Transform,
    point: Vec3,
    reach_mm: Scalar,
    to: Snap,
) -> (Vec3, Snapped) {
    let free = (point, Snapped::Free);
    let matrix = transform.to_matrix();
    let inverse = matrix.inverse();
    if to == Snap::Free || !inverse.is_finite() {
        return free;
    }
    let Some(near) = bvh.closest(inverse.transform_point3(point)) else {
        return free;
    };
    let placed = Placed { mesh, matrix };
    [reach_mm, reach_mm * WIDER]
        .into_iter()
        .find_map(|reach_mm| placed.caught(adjacency, near.face, point, reach_mm, to))
        .unwrap_or(free)
}

/// How much further a snap looks when nothing turns within its reach.
const WIDER: Scalar = 4.0;

/// A mesh where its model stands.
struct Placed<'a> {
    mesh: &'a Mesh,
    matrix: Mat4,
}

impl Placed<'_> {
    /// The nearest corner or point of an edge where the surface turns, within `reach_mm`.
    fn caught(
        &self,
        adjacency: &Adjacency,
        seed: usize,
        point: Vec3,
        reach_mm: Scalar,
        to: Snap,
    ) -> Option<(Vec3, Snapped)> {
        let around = self.around(adjacency, seed, point, reach_mm);
        let edges = around.iter().flat_map(|face| {
            sides(self.mesh, *face)
                .filter(|(a, b)| is_feature(self.mesh, adjacency, *face, *a, *b))
                .filter_map(|(a, b)| Some((self.corner(a)?, self.corner(b)?)))
        });
        let (candidates, snapped): (Vec<Vec3>, _) = match to {
            Snap::Corner => (edges.flat_map(|(a, b)| [a, b]).collect(), Snapped::Corner),
            _ => (
                edges.map(|(a, b)| on_segment(point, a, b)).collect(),
                Snapped::Edge,
            ),
        };
        candidates
            .into_iter()
            .map(|candidate| (candidate, (candidate - point).length()))
            .filter(|(_, distance)| *distance <= reach_mm)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(candidate, _)| (candidate, snapped))
    }

    fn corner(&self, index: u32) -> Option<Vec3> {
        let vertex = self.mesh.vertices.get(index as usize)?;
        Some(self.matrix.transform_point3(*vertex))
    }

    fn triangle(&self, face: usize) -> Option<Triangle> {
        let [a, b, c] = *self.mesh.faces.get(face)?;
        Some(Triangle::new(
            self.corner(a)?,
            self.corner(b)?,
            self.corner(c)?,
        ))
    }

    /// The triangles within `reach_mm` of `point`, walked across shared edges from `seed`.
    fn around(
        &self,
        adjacency: &Adjacency,
        seed: usize,
        point: Vec3,
        reach_mm: Scalar,
    ) -> Vec<usize> {
        let mut seen = FastSet::default();
        seen.insert(seed);
        let mut queue = vec![seed];
        let mut found = Vec::new();
        while let Some(face) = queue.pop() {
            let within = self.triangle(face).is_some_and(|triangle| {
                point_triangle(point, &triangle).distance(point) <= reach_mm
            });
            if !within || found.len() >= MAX_AROUND {
                continue;
            }
            found.push(face);
            for next in adjacency.neighbours(face) {
                if seen.insert(*next as usize) {
                    queue.push(*next as usize);
                }
            }
        }
        found
    }
}

/// The three edges of `face`, each as the two corners it runs between.
fn sides(mesh: &Mesh, face: usize) -> impl Iterator<Item = (u32, u32)> + '_ {
    let corners = mesh.faces.get(face).copied();
    (0..3).filter_map(move |side| {
        let corners = corners?;
        Some((corners[side], corners[(side + 1) % 3]))
    })
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
    fn clearing_forgets_both_ends_and_keeps_the_settings() {
        let mut measure = Measure {
            snap: Snap::Edge,
            show_axes: true,
            ..Measure::default()
        };
        measure.pick(Vec3::ZERO, Snapped::Free);
        measure.pick(Vec3::X, Snapped::Free);
        measure.clear();
        assert!(measure.span().is_none() && measure.start().is_none());
        assert_eq!((measure.snap, measure.show_axes), (Snap::Edge, true));
    }

    #[test]
    fn alt_measures_freely_whatever_is_chosen() {
        let measure = Measure {
            snap: Snap::Corner,
            ..Measure::default()
        };
        assert_eq!(measure.snap_now(true), Snap::Free);
        assert_eq!(
            measure.snap_now(false),
            Snap::Corner,
            "let go, the choice is back"
        );
    }

    #[test]
    fn a_corner_snap_passes_over_an_edge_for_its_corner() {
        let point = Vec3::new(0.4, 0.1, 1.0);
        let (snapped, by) = snap_on_cube(Transform::default(), point, Snap::Corner);
        assert_eq!(by, Snapped::Corner);
        assert!(
            snapped.abs_diff_eq(Vec3::new(0.0, 0.0, 1.0), 1e-5),
            "the nearest corner of the top, got {snapped}"
        );
    }

    #[test]
    fn an_edge_snap_passes_over_a_corner_for_its_edge() {
        let (snapped, by) =
            snap_on_cube(Transform::default(), Vec3::new(0.95, 0.9, 1.0), Snap::Edge);
        assert_eq!(by, Snapped::Edge);
        assert!(
            snapped.abs_diff_eq(Vec3::new(1.0, 0.9, 1.0), 1e-5),
            "the point on the right edge straight across, got {snapped}"
        );
    }

    fn cube_object(scale: Scalar) -> SceneObject {
        let mesh = Arc::new(unit_cube());
        let summary = crate::scene::ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: core_geometry::Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: core_geometry::diagnose(&mesh),
        };
        let transform = Transform {
            scale: Vec3::splat(scale),
            ..Transform::default()
        };
        let mut scene = crate::scene::Scene::default();
        let id = scene.insert(crate::scene::Imported::new(
            "cube".to_owned(),
            mesh,
            transform,
            summary,
        ));
        scene.get(id).expect("just inserted").clone()
    }

    #[test]
    fn opposite_sides_of_a_cube_are_parallel_its_side_apart() {
        let cube = cube_object(10.0);
        let adjacency = Adjacency::of(&cube.mesh);
        let (bottom, top) = (0, 2);
        let mut measure = Measure::default();
        measure.pick_plane(Plane::of(&cube, &adjacency, bottom).expect("a flat side"));
        measure.pick_plane(Plane::of(&cube, &adjacency, top).expect("a flat side"));

        let gap = measure.plane_gap().expect("two planes picked");
        assert!(
            gap.is_parallel(),
            "opposite sides, got {} deg",
            gap.angle_deg
        );
        assert!(
            (gap.gap_mm - 10.0).abs() < 1e-4,
            "a 10 mm cube is 10 mm through, got {}",
            gap.gap_mm
        );
    }

    #[test]
    fn neighbouring_sides_of_a_cube_stand_square() {
        let cube = cube_object(1.0);
        let adjacency = Adjacency::of(&cube.mesh);
        let (bottom, front) = (0, 4);
        let mut measure = Measure::default();
        measure.pick_plane(Plane::of(&cube, &adjacency, bottom).expect("a flat side"));
        measure.pick_plane(Plane::of(&cube, &adjacency, front).expect("a flat side"));

        let gap = measure.plane_gap().expect("two planes picked");
        assert!(
            (gap.angle_deg - 90.0).abs() < 1e-3,
            "sides of a cube meet square, got {}",
            gap.angle_deg
        );
        assert!(!gap.is_parallel());
        assert_eq!(
            (gap.from, gap.to),
            (Vec3::new(0.5, 0.0, 0.5), Vec3::new(0.5, 0.5, 0.0)),
            "a tilted pair is drawn centre to centre"
        );
    }

    /// Snaps on the unit cube, placed by `transform`, within half a millimetre.
    fn snap_on_cube(transform: Transform, point: Vec3, to: Snap) -> (Vec3, Snapped) {
        let mesh = Arc::new(unit_cube());
        let bvh = Bvh::build(&mesh);
        snap(
            &mesh,
            &bvh,
            &Adjacency::of(&mesh),
            transform,
            point,
            0.5,
            to,
        )
    }

    #[test]
    fn a_click_near_a_corner_measures_the_corner() {
        let near_corner = Vec3::new(0.98, 0.99, 1.0);

        let (snapped, by) = snap_on_cube(Transform::default(), near_corner, Snap::Corner);
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

        let (snapped, by) = snap_on_cube(ten, middle, Snap::Edge);
        assert_eq!(by, Snapped::Free);
        assert!(
            snapped.abs_diff_eq(middle, 1e-6),
            "no edge is within reach, and the diagonal seam is none"
        );
    }

    #[test]
    fn a_corner_is_measured_where_the_model_stands() {
        let placed = Transform {
            translation: Vec3::new(10.0, 0.0, 0.0),
            scale: Vec3::splat(2.0),
            ..Transform::default()
        };

        let (snapped, _) = snap_on_cube(placed, Vec3::new(11.9, 1.9, 2.0), Snap::Corner);
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
        assert_eq!(
            snap_on_cube(flat, point, Snap::Edge),
            (point, Snapped::Free)
        );
    }

    #[test]
    fn an_edge_out_of_reach_is_still_found_further_out() {
        // 0.8 mm in from the front edge: past the 0.5 mm reach, inside four times it.
        let ten = Transform {
            scale: Vec3::splat(10.0),
            ..Transform::default()
        };
        let (snapped, by) = snap_on_cube(ten, Vec3::new(5.0, 0.8, 10.0), Snap::Edge);
        assert_eq!(by, Snapped::Edge);
        assert!(
            snapped.abs_diff_eq(Vec3::new(5.0, 0.0, 10.0), 1e-4),
            "got {snapped}"
        );
    }

    #[test]
    fn a_click_near_an_edge_measures_the_edge() {
        // 0.1 mm in from the top front edge of the cube, half way along it.
        let (snapped, by) =
            snap_on_cube(Transform::default(), Vec3::new(0.5, 0.1, 1.0), Snap::Edge);
        assert_eq!(by, Snapped::Edge);
        assert!(
            snapped.abs_diff_eq(Vec3::new(0.5, 0.0, 1.0), 1e-5),
            "the point on the edge straight across, got {snapped}"
        );
    }
}

use core_geometry::{Adjacency, FastMap, Mat3, Mesh, Scalar, Vec2, Vec3};

use serde::{Deserialize, Serialize};

use crate::placed::Placed;

use crate::region::Region;

/// How close two contacts of a fill may come, as a fraction of the finest spacing it was
/// asked for. Under one, so that a run of points laid down at exactly that spacing keeps
/// every one of them.
const CROWD: Scalar = 0.75;

/// How far outside a triangle, as a fraction of it, a sample still counts as on its edge.
const ON_EDGE: Scalar = 1e-5;

/// A spacing under this would fill a patch with more supports than resin; it is a tenth
/// of the narrowest head any profile ships.
const MIN_SPACING_MM: Scalar = 0.05;

/// A transform this close to singular has flattened its model, and a face of it has no
/// normal on the plate to measure.
const SINGULAR: Scalar = 1e-12;

/// How a painted patch is filled with supports.
///
/// The rim and the inside are asked for separately, because what an overhang needs held
/// is its edge first: that is where a layer has nothing under it at all.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    /// Spacing of the grid laid over the patch, millimetres, or `None` to leave the
    /// inside of it bare.
    pub infill_spacing_mm: Option<Scalar>,
    /// Spacing of the points put along the patch's rim, millimetres, or `None` for a
    /// patch with nothing on its edge.
    pub border_spacing_mm: Option<Scalar>,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            infill_spacing_mm: Some(3.0),
            border_spacing_mm: Some(2.0),
        }
    }
}

/// Where supports go to hold up `region` of the model, in plate coordinates.
///
/// A face the same model has blocked is left out of the fill, and so is one leaning
/// further from a ceiling than `max_overhang_deg`: paint says where a support may go, the
/// profile's overhang angle says whether one is needed. So painting over a patch that is
/// partly forbidden, or over a wall that holds itself up, fills the rest of it.
///
/// The inside is sampled on a grid of the plate rather than over the triangles, so the
/// spacing is the spacing the print sees however finely the patch is meshed, and the rim
/// is walked along the edges the fill ends on — the boundary of a set of faces, not an
/// offset polygon. See `docs/decisions/0092`.
pub fn project(
    placed: &Placed,
    adjacency: &Adjacency,
    region: &Region,
    settings: &ProjectSettings,
    seeds: &[Vec3],
    max_overhang_deg: Scalar,
) -> Vec<Vec3> {
    let matrix = placed.transform.to_matrix();
    let rim_mm = settings
        .border_spacing_mm
        .filter(|spacing| *spacing >= MIN_SPACING_MM);
    let pitch_mm = settings
        .infill_spacing_mm
        .filter(|spacing| *spacing >= MIN_SPACING_MM);
    let Some(apart_mm) = rim_mm
        .into_iter()
        .chain(pitch_mm)
        .fold(None, |least: Option<Scalar>, spacing| {
            Some(least.map_or(spacing, |least| least.min(spacing)))
        })
        .map(|least| least * CROWD)
    else {
        return Vec::new();
    };

    let region = &hanging(placed, region, max_overhang_deg);

    // The supports already standing come first, so a second fill of the same patch adds
    // nothing on top of the first.
    let mut taken: FastMap<[i32; 3], Vec<Vec3>> = FastMap::default();
    for seed in seeds {
        take_apart(&mut taken, *seed, apart_mm);
    }

    let mut points = Vec::new();
    // The rim goes down first: a patch's edge is where a layer has nothing under it at
    // all, and the grid inside it gives way rather than the other way round.
    if let Some(spacing_mm) = rim_mm {
        for point in border_points(placed, adjacency, region, spacing_mm) {
            let placed = matrix.transform_point3(point);
            if take_apart(&mut taken, placed, apart_mm) {
                points.push(placed);
            }
        }
    }
    if let Some(pitch_mm) = pitch_mm {
        for point in infill_points(placed, region, pitch_mm) {
            if take_apart(&mut taken, point, apart_mm) {
                points.push(point);
            }
        }
    }
    points
}

/// The faces of `region` a support may be put under: the ones the model has not blocked,
/// hanging over nothing as far as `max_overhang_deg` says.
///
/// A face's lean is the downward part of its own normal on the plate, which is the test
/// the viewport washes an overhang with; see `docs/decisions/0034`.
fn hanging(placed: &Placed, region: &Region, max_overhang_deg: Scalar) -> Region {
    let matrix = placed.transform.to_matrix();
    if matrix.determinant().abs() < SINGULAR {
        return Region::default();
    }
    // A normal does not survive a non-uniform scale under the model matrix, but it does
    // under the inverse transpose of that matrix's upper 3x3.
    let normals = Mat3::from_mat4(matrix.inverse()).transpose();
    let holds_itself_up = max_overhang_deg.clamp(0.0, 90.0).to_radians().sin();

    let mut hanging = Region::default();
    for face in region.faces().filter(|face| !placed.blocks_face(*face)) {
        let Some(triangle) = placed.model.triangle(face) else {
            continue;
        };
        let lean = -(normals * triangle.normal_unnormalized())
            .normalize_or_zero()
            .z;
        if lean >= holds_itself_up {
            hanging.set(face, true);
        }
    }
    hanging
}

/// The lowest point of the patch over each cell of a grid `pitch_mm` across.
///
/// Lowest because a patch that folds over itself is being held from underneath: the
/// support has to reach the first surface it meets on the way up, not the last.
fn infill_points(placed: &Placed, region: &Region, pitch_mm: Scalar) -> Vec<Vec3> {
    let matrix = placed.transform.to_matrix();
    let mut lowest: FastMap<[i32; 2], Scalar> = FastMap::default();
    for face in region.faces() {
        let Some(triangle) = placed.model.triangle(face) else {
            continue;
        };
        let corners = [triangle.a, triangle.b, triangle.c].map(|corner| {
            let placed = matrix.transform_point3(corner);
            (placed.truncate(), placed.z)
        });
        let flat = corners.map(|(at, _)| at);
        let min = flat.iter().fold(flat[0], |low, at| low.min(*at));
        let max = flat.iter().fold(flat[0], |high, at| high.max(*at));

        for x in cell_range(min.x, max.x, pitch_mm) {
            for y in cell_range(min.y, max.y, pitch_mm) {
                let at = centre_of([x, y], pitch_mm);
                let Some(weights) = barycentric(flat, at) else {
                    continue;
                };
                let z = (0..3)
                    .map(|corner| weights[corner] * corners[corner].1)
                    .sum();
                lowest
                    .entry([x, y])
                    .and_modify(|held| *held = held.min(z))
                    .or_insert(z);
            }
        }
    }

    lowest
        .into_iter()
        .map(|(cell, z)| centre_of(cell, pitch_mm).extend(z))
        .collect()
}

/// Points every `spacing_mm` along the edges the region ends on, in the model's own
/// space, before they are thinned against each other.
///
/// An edge is on the rim when the faces across it are outside the region, which includes
/// the open edge of a shell that has none. A fine mesh has many rim edges shorter than
/// the spacing, so each one still offers its ends and [`project`] is what keeps the rim
/// from becoming one support a triangle.
fn border_points(
    placed: &Placed,
    adjacency: &Adjacency,
    region: &Region,
    spacing_mm: Scalar,
) -> Vec<Vec3> {
    let model = placed.model;
    let mut points = Vec::new();
    for face in region.faces() {
        let Some(corners) = model.faces.get(face) else {
            continue;
        };
        for corner in 0..3 {
            let edge = (corners[corner], corners[(corner + 1) % 3]);
            if shared_with_region(model, adjacency, region, face, edge) {
                continue;
            }
            let (from, to) = (
                model.vertices[edge.0 as usize],
                model.vertices[edge.1 as usize],
            );
            let steps = (from.distance(to) / spacing_mm).floor() as usize;
            points.extend(
                (0..=steps).map(|step| from.lerp(to, step as Scalar / steps.max(1) as Scalar)),
            );
        }
    }
    points
}

/// Takes `point` unless something already taken is within `spacing_mm` of it, keeping the
/// taken points in buckets that size so the question reads twenty-seven of them.
fn take_apart(taken: &mut FastMap<[i32; 3], Vec<Vec3>>, point: Vec3, spacing_mm: Scalar) -> bool {
    let cell = (point / spacing_mm).floor().as_ivec3().to_array();
    let crowded = (-1..=1).any(|x| {
        (-1..=1).any(|y| {
            (-1..=1).any(|z| {
                taken
                    .get(&[cell[0] + x, cell[1] + y, cell[2] + z])
                    .is_some_and(|near| near.iter().any(|other| other.distance(point) < spacing_mm))
            })
        })
    });
    if crowded {
        return false;
    }
    taken.entry(cell).or_default().push(point);
    true
}

/// Whether a face of the region other than `face` also has `edge`, which makes it an edge
/// inside the region rather than on its rim.
fn shared_with_region(
    model: &Mesh,
    adjacency: &Adjacency,
    region: &Region,
    face: usize,
    edge: (u32, u32),
) -> bool {
    adjacency
        .neighbours(face)
        .iter()
        .filter(|other| region.contains(**other as usize))
        .filter_map(|other| model.faces.get(*other as usize))
        .any(|corners| corners.contains(&edge.0) && corners.contains(&edge.1))
}

fn centre_of(cell: [i32; 2], pitch_mm: Scalar) -> Vec2 {
    Vec2::new(
        (cell[0] as Scalar + 0.5) * pitch_mm,
        (cell[1] as Scalar + 0.5) * pitch_mm,
    )
}

fn cell_range(from_mm: Scalar, to_mm: Scalar, pitch_mm: Scalar) -> std::ops::RangeInclusive<i32> {
    (from_mm / pitch_mm).floor() as i32..=(to_mm / pitch_mm).floor() as i32
}

/// Barycentric weights of `at` inside the triangle `corners`, or `None` when it is
/// outside it or the triangle has no area on the plate.
///
/// The weights are allowed a hair under zero, or a sample landing exactly on the edge two
/// triangles share would be taken by neither.
fn barycentric(corners: [Vec2; 3], at: Vec2) -> Option<[Scalar; 3]> {
    let (ab, ac) = (corners[1] - corners[0], corners[2] - corners[0]);
    let area = ab.perp_dot(ac);
    if area.abs() < Scalar::EPSILON {
        return None;
    }
    let to = at - corners[0];
    let beta = to.perp_dot(ac) / area;
    let gamma = ab.perp_dot(to) / area;
    let alpha = 1.0 - beta - gamma;
    [alpha, beta, gamma]
        .iter()
        .all(|weight| *weight >= -ON_EDGE)
        .then_some([alpha, beta, gamma])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::box_mesh;
    use core_geometry::{Bvh, Transform};

    /// The overhang angle the shipped profiles are around, so a ceiling is held and a
    /// wall is not.
    const OVERHANG_DEG: Scalar = 45.0;

    /// A model standing where it was modelled, with nothing blocked.
    fn standing<'a>(model: &'a Mesh, bvh: &'a Bvh) -> Placed<'a> {
        Placed::new(model, bvh, Transform::default())
    }

    /// A 10 mm square of two triangles lying flat at `z`, wound so it looks down: the
    /// ceiling a patch fill is really for.
    fn ceiling(z: Scalar) -> Mesh {
        Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, z),
                Vec3::new(10.0, 0.0, z),
                Vec3::new(10.0, 10.0, z),
                Vec3::new(0.0, 10.0, z),
            ],
            vec![[0, 2, 1], [0, 3, 2]],
        )
    }

    fn whole(model: &Mesh) -> Region {
        let mut region = Region::default();
        for face in 0..model.faces.len() {
            region.set(face, true);
        }
        region
    }

    fn filled(model: &Mesh, settings: &ProjectSettings) -> Vec<Vec3> {
        let bvh = Bvh::build(model);
        project(
            &standing(model, &bvh),
            &Adjacency::of(model),
            &whole(model),
            settings,
            &[],
            OVERHANG_DEG,
        )
    }

    #[test]
    fn nothing_painted_is_nothing_to_hold_up() {
        let model = ceiling(5.0);
        let bvh = Bvh::build(&model);
        let points = project(
            &standing(&model, &bvh),
            &Adjacency::of(&model),
            &Region::default(),
            &ProjectSettings::default(),
            &[],
            OVERHANG_DEG,
        );
        assert!(points.is_empty());
    }

    #[test]
    fn a_square_patch_is_filled_on_the_grid_it_was_asked_for() {
        let points = filled(
            &ceiling(5.0),
            &ProjectSettings {
                infill_spacing_mm: Some(2.0),
                border_spacing_mm: None,
            },
        );

        // Cell centres at 1, 3, 5, 7 and 9 mm along each side of a 10 mm square.
        assert_eq!(points.len(), 25);
        assert!(
            points.iter().all(|point| (point.z - 5.0).abs() < 1e-4),
            "every point sits on the patch it fills"
        );
    }

    #[test]
    fn a_patch_is_filled_where_the_model_stands_not_where_it_was_modelled() {
        let model = ceiling(5.0);
        let bvh = Bvh::build(&model);
        let moved = Placed::new(
            &model,
            &bvh,
            Transform::from_translation(Vec3::new(40.0, 0.0, 0.0)),
        );
        let points = project(
            &moved,
            &Adjacency::of(&model),
            &whole(&model),
            &ProjectSettings {
                infill_spacing_mm: Some(2.0),
                border_spacing_mm: None,
            },
            &[],
            OVERHANG_DEG,
        );

        assert_eq!(points.len(), 25);
        assert!(
            points.iter().all(|point| point.x >= 40.0),
            "the fill followed the model across the plate"
        );
    }

    #[test]
    fn the_rim_of_a_patch_is_walked_at_its_own_spacing() {
        let points = filled(
            &ceiling(5.0),
            &ProjectSettings {
                infill_spacing_mm: None,
                border_spacing_mm: Some(2.0),
            },
        );

        // Four 10 mm sides walked every 2 mm, the corners counted once each.
        assert_eq!(points.len(), 20);
        assert!(
            points
                .iter()
                .all(|point| point.x.min(point.y) <= 1e-4 || point.x.max(point.y) >= 10.0 - 1e-4),
            "every rim point is on an edge of the square"
        );
    }

    /// The same square, meshed as a fan of many small triangles, looking down like the
    /// square it stands for: the rim is then made of edges far shorter than the spacing
    /// asked for.
    fn fine_ceiling(z: Scalar, steps: usize) -> Mesh {
        let mut vertices = vec![Vec3::new(5.0, 5.0, z)];
        for step in 0..steps {
            let along = 40.0 * step as Scalar / steps as Scalar;
            let (x, y) = match along {
                a if a < 10.0 => (a, 0.0),
                a if a < 20.0 => (10.0, a - 10.0),
                a if a < 30.0 => (30.0 - a, 10.0),
                a => (0.0, 40.0 - a),
            };
            vertices.push(Vec3::new(x, y, z));
        }
        let faces = (0..steps)
            .map(|step| [0, 1 + ((step + 1) % steps) as u32, 1 + step as u32])
            .collect();
        Mesh::new(vertices, faces)
    }

    #[test]
    fn a_finely_meshed_rim_is_walked_at_the_spacing_not_at_the_triangle() {
        let points = filled(
            &fine_ceiling(5.0, 200),
            &ProjectSettings {
                infill_spacing_mm: None,
                border_spacing_mm: Some(2.0),
            },
        );

        // 40 mm of rim every 2 mm, give or take where the corners fall.
        assert!(
            (16..=24).contains(&points.len()),
            "a rim of 200 short edges became {} supports",
            points.len()
        );
    }

    #[test]
    fn the_diagonal_the_square_is_meshed_along_is_not_a_rim() {
        let points = filled(
            &ceiling(5.0),
            &ProjectSettings {
                infill_spacing_mm: None,
                border_spacing_mm: Some(2.0),
            },
        );
        assert!(
            !points
                .iter()
                .any(|point| (point.x - 5.0).abs() < 1e-4 && (point.y - 5.0).abs() < 1e-4),
            "the shared edge of the two triangles is inside the patch"
        );
    }

    #[test]
    fn a_rim_point_keeps_the_grid_from_crowding_it() {
        let points = filled(
            &ceiling(5.0),
            &ProjectSettings {
                infill_spacing_mm: Some(2.0),
                border_spacing_mm: Some(2.0),
            },
        );
        let close = points.iter().enumerate().any(|(at, point)| {
            points[at + 1..]
                .iter()
                .any(|other| point.distance(*other) < 1.0)
        });
        assert!(!close, "no two supports were put within a millimetre");
    }

    #[test]
    fn a_blocked_face_is_left_out_of_the_fill() {
        let model = ceiling(5.0);
        let bvh = Bvh::build(&model);
        let mut blocked_region = Region::default();
        blocked_region.set(0, true);
        let blocked = crate::Blocked::new(&model, &blocked_region, Transform::default())
            .expect("half the square was blocked");
        let placed = standing(&model, &bvh).blocking(Some(&blocked));

        let points = project(
            &placed,
            &Adjacency::of(&model),
            &whole(&model),
            &ProjectSettings {
                infill_spacing_mm: Some(2.0),
                border_spacing_mm: None,
            },
            &[],
            OVERHANG_DEG,
        );

        assert!(
            points.iter().all(|point| point.y >= point.x - 1e-4),
            "the blocked half of the square was filled anyway"
        );
        assert!(!points.is_empty(), "the half that is free is still filled");
    }

    #[test]
    fn a_fill_puts_nothing_on_top_of_a_support_already_standing() {
        let model = ceiling(5.0);
        let bvh = Bvh::build(&model);
        let settings = ProjectSettings {
            infill_spacing_mm: Some(2.0),
            border_spacing_mm: None,
        };
        let once = project(
            &standing(&model, &bvh),
            &Adjacency::of(&model),
            &whole(&model),
            &settings,
            &[],
            OVERHANG_DEG,
        );
        let twice = project(
            &standing(&model, &bvh),
            &Adjacency::of(&model),
            &whole(&model),
            &settings,
            &once,
            OVERHANG_DEG,
        );

        assert!(
            twice.is_empty(),
            "filling the same patch again added {} more supports",
            twice.len()
        );
    }

    /// A 10 mm square standing on its edge, wound so it looks along x: a wall holds
    /// itself up whatever is painted on it.
    fn wall() -> Mesh {
        Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 10.0, 0.0),
                Vec3::new(0.0, 10.0, 10.0),
                Vec3::new(0.0, 0.0, 10.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        )
    }

    #[test]
    fn a_painted_wall_is_not_filled() {
        let points = filled(
            &wall(),
            &ProjectSettings {
                infill_spacing_mm: Some(2.0),
                border_spacing_mm: Some(2.0),
            },
        );
        assert!(
            points.is_empty(),
            "a wall leaning nothing at all took {} supports at {OVERHANG_DEG} degrees",
            points.len()
        );
    }

    #[test]
    fn a_wall_is_filled_once_the_angle_asks_for_it() {
        let model = wall();
        let bvh = Bvh::build(&model);
        let points = project(
            &standing(&model, &bvh),
            &Adjacency::of(&model),
            &whole(&model),
            &ProjectSettings {
                infill_spacing_mm: None,
                border_spacing_mm: Some(2.0),
            },
            &[],
            0.0,
        );
        assert!(
            !points.is_empty(),
            "an angle of zero holds up every face that is painted"
        );
    }

    #[test]
    fn a_patch_that_folds_over_itself_is_held_from_underneath() {
        let model = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
        let points = filled(
            &model,
            &ProjectSettings {
                infill_spacing_mm: Some(4.0),
                border_spacing_mm: None,
            },
        );
        assert!(
            points.iter().all(|point| point.z < 1e-4),
            "a cube painted all over is filled on its underside, not on its lid"
        );
    }
}

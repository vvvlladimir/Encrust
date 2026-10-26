use core_geometry::{
    Adjacency, FastMap, Mat3, Mesh, Region, Scalar, Vec2, Vec3, cell_of_point, cells_around,
};

use serde::{Deserialize, Serialize};

use crate::placed::Placed;

mod footprint;

use footprint::Footprint;

/// How close two contacts of a fill may come, as a fraction of the finest spacing asked for;
/// under one, so points laid at exactly that spacing all survive.
const CROWD: Scalar = 0.75;

/// How far outside a triangle, as a fraction of it, a sample still counts as on its edge.
const ON_EDGE: Scalar = 1e-5;

/// A spacing under this would fill a patch with more supports than resin; it is a tenth
/// of the narrowest head any profile ships.
const MIN_SPACING_MM: Scalar = 0.05;

/// How a painted patch is filled with supports; rim and inside are asked separately since an
/// overhang needs its edge held first. Offsets are in from the outline seen from the plate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    /// Spacing of the points laid over the inside of the patch, millimetres, or `None` to
    /// leave the inside of it bare.
    pub infill_spacing_mm: Option<Scalar>,
    /// Spacing of the points put along the patch's rim, millimetres, or `None` for a
    /// patch with nothing on its edge.
    pub border_spacing_mm: Option<Scalar>,
    /// How far in from the outline the rim stands, millimetres; nought walks the edges the
    /// patch ends on. A file without it reads nought, which is what it was filled with.
    #[serde(default)]
    pub border_offset_mm: Scalar,
    /// How far inside the rim a second rim stands, millimetres, or `None` for one rim.
    #[serde(default)]
    pub second_border_mm: Option<Scalar>,
    /// How far inside the innermost rim, or the outline with no rim, the inside begins,
    /// millimetres.
    #[serde(default)]
    pub infill_offset_mm: Scalar,
    #[serde(default)]
    pub pattern: InfillPattern,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            infill_spacing_mm: Some(3.0),
            border_spacing_mm: Some(2.0),
            border_offset_mm: 1.0,
            second_border_mm: None,
            infill_offset_mm: 1.0,
            pattern: InfillPattern::Grid,
        }
    }
}

/// How the inside of a patch is laid out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfillPattern {
    /// Rows and columns of the plate's own grid, the spacing apart.
    #[default]
    Grid,
    /// Rings following the rim inwards, the spacing apart along and across them.
    Concentric,
    /// Scattered no closer than the spacing, the same way each time.
    Random,
}

/// The contacts a fill puts down, in plate coordinates: the rim's apart from the
/// inside's, since each may be built to a profile of its own.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Projected {
    pub rim: Vec<Vec3>,
    pub inside: Vec<Vec3>,
}

/// Where supports go to hold up `region`, in plate coordinates; blocked faces and faces leaning
/// past `max_overhang_deg` are left out. A rim at nought walks the fill's edges (ADR 0092, 0331).
pub fn project(
    placed: &Placed,
    adjacency: &Adjacency,
    region: &Region,
    settings: &ProjectSettings,
    seeds: &[Vec3],
    max_overhang_deg: Scalar,
) -> Projected {
    let rim_mm = settings
        .border_spacing_mm
        .filter(|spacing| *spacing >= MIN_SPACING_MM);
    let pitch_mm = settings
        .infill_spacing_mm
        .filter(|spacing| *spacing >= MIN_SPACING_MM);
    let Some(finest_mm) = rim_mm
        .into_iter()
        .chain(pitch_mm)
        .fold(None, |least: Option<Scalar>, spacing| {
            Some(least.map_or(spacing, |least| least.min(spacing)))
        })
    else {
        return Projected::default();
    };
    let apart_mm = finest_mm * CROWD;

    let region = &hanging(placed, region, max_overhang_deg);
    let rings = rim_rings(settings, rim_mm.is_some());
    let inside_from_mm = rings.last().copied().unwrap_or(0.0) + settings.infill_offset_mm.max(0.0);
    let footprint = (rings.iter().any(|ring| *ring > 0.0)
        || (pitch_mm.is_some()
            && (inside_from_mm > 0.0 || settings.pattern != InfillPattern::Grid)))
        .then(|| Footprint::of(placed, region, finest_mm))
        .flatten();

    // The supports already standing come first, so a second fill of the same patch adds
    // nothing on top of the first.
    let mut taken: FastMap<[i32; 3], Vec<Vec3>> = FastMap::default();
    for seed in seeds {
        take_apart(&mut taken, *seed, apart_mm);
    }

    let mut projected = Projected::default();
    // The rim goes down first: a patch's edge is where a layer has nothing under it at
    // all, and the inside gives way rather than the other way round.
    if let Some(spacing_mm) = rim_mm {
        for ring in &rings {
            let points = match &footprint {
                Some(footprint) if *ring > 0.0 => footprint.ring(*ring, spacing_mm),
                _ => edge_points(placed, adjacency, region, spacing_mm),
            };
            projected.rim.extend(
                points
                    .into_iter()
                    .filter(|point| take_apart(&mut taken, *point, apart_mm)),
            );
        }
    }
    if let Some(pitch_mm) = pitch_mm {
        let points = inside_points(
            placed,
            region,
            footprint.as_ref(),
            settings.pattern,
            inside_from_mm,
            pitch_mm,
        );
        projected.inside.extend(
            points
                .into_iter()
                .filter(|point| take_apart(&mut taken, *point, apart_mm)),
        );
    }
    projected
}

/// How far in from the outline each rim stands, millimetres: none without a rim, else the
/// first and the second if one is asked for.
fn rim_rings(settings: &ProjectSettings, rim: bool) -> Vec<Scalar> {
    if !rim {
        return Vec::new();
    }
    let first = settings.border_offset_mm.max(0.0);
    std::iter::once(first)
        .chain(
            settings
                .second_border_mm
                .map(|gap| first + gap.max(MIN_SPACING_MM)),
        )
        .collect()
}

/// The inside of the patch laid out in `pattern`, from `from_mm` inside the outline.
fn inside_points(
    placed: &Placed,
    region: &Region,
    footprint: Option<&Footprint>,
    pattern: InfillPattern,
    from_mm: Scalar,
    pitch_mm: Scalar,
) -> Vec<Vec3> {
    let Some(footprint) = footprint else {
        return infill_points(placed, region, pitch_mm);
    };
    match pattern {
        // A grid point the footprint cannot see under has nothing it is inside of.
        InfillPattern::Grid => infill_points(placed, region, pitch_mm)
            .into_iter()
            .filter(|point| {
                footprint.depth_at(point.truncate()) + footprint.cell_mm() / 2.0 >= from_mm
            })
            .collect(),
        InfillPattern::Concentric => {
            let mut points = Vec::new();
            for ring in 0.. {
                let ring = footprint.ring(from_mm + ring as Scalar * pitch_mm, pitch_mm);
                if ring.is_empty() {
                    break;
                }
                points.extend(ring);
            }
            points
        }
        InfillPattern::Random => footprint.scattered(from_mm, pitch_mm),
    }
}

/// The faces of `region` a support may be put under: not blocked, hanging over nothing as far
/// as `max_overhang_deg` says, by the lean the viewport washes overhangs with (ADR 0034).
fn hanging(placed: &Placed, region: &Region, max_overhang_deg: Scalar) -> Region {
    let matrix = placed.transform.to_matrix();
    let Some(inverse) = placed.transform.inverse_matrix() else {
        return Region::default();
    };
    // A normal does not survive a non-uniform scale under the model matrix, but it does
    // under the inverse transpose of that matrix's upper 3x3.
    let normals = Mat3::from_mat4(inverse).transpose();
    let holds_itself_up = max_overhang_deg.clamp(0.0, 90.0).to_radians().sin();

    let mut hanging = Region::default();
    for face in region.faces() {
        let Some(triangle) = placed.model.triangle(face) else {
            continue;
        };
        let middle = (triangle.a + triangle.b + triangle.c) / 3.0;
        if placed.blocks_at(matrix.transform_point3(middle)) {
            continue;
        }
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
fn infill_points(placed: &Placed, region: &Region, pitch_mm: Scalar) -> Vec<Vec3> {
    lowest_surface(placed, region, pitch_mm)
        .into_iter()
        .map(|(cell, z)| centre_of(cell, pitch_mm).extend(z))
        .collect()
}

/// The height of the patch's lowest surface over each cell middle of a `pitch_mm` grid; lowest
/// since a folded patch is held from underneath and the support meets the first surface.
fn lowest_surface(placed: &Placed, region: &Region, pitch_mm: Scalar) -> FastMap<[i32; 2], Scalar> {
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
}

/// Points every `spacing_mm` along the edges the region ends on, in plate coordinates.
fn edge_points(
    placed: &Placed,
    adjacency: &Adjacency,
    region: &Region,
    spacing_mm: Scalar,
) -> Vec<Vec3> {
    let matrix = placed.transform.to_matrix();
    border_points(placed, adjacency, region, spacing_mm)
        .into_iter()
        .map(|point| matrix.transform_point3(point))
        .collect()
}

/// Points every `spacing_mm` along the edges the region ends on, in model space, before thinning;
/// open shell edges count, and each short edge offers its ends for [`project`] to thin.
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
    let cell = cell_of_point(point, spacing_mm);
    let crowded = cells_around(cell).any(|around| {
        taken
            .get(&around)
            .is_some_and(|near| near.iter().any(|other| other.distance(point) < spacing_mm))
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

/// Barycentric weights of `at` inside the triangle `corners`, or `None` when outside it or the
/// triangle has no area on the plate; a hair under zero is allowed so shared edges are not missed.
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
mod tests;

//! What the supports are, read off their geometry rather than the cured stack: how much
//! each tip carries, what is left bare, and what a tip will scar or break when it comes off.

use core_geometry::{
    Bvh, FastMap, Mesh, Quat, Ray, Scalar, Triangle, Vec3, diagnose, signed_volume,
};
use core_plate::LOOKS_UP;
use printer_profiles::{ContactShape, SupportProfile};
use serde::{Deserialize, Serialize};

use crate::stand::Supported;

/// A tip on material thinner than this behind it, millimetres, is on a detail: a finger, an
/// ear, a blade. Taking it off is where details break.
const THIN_DETAIL_MM: Scalar = 1.0;
/// A tip carrying more than this many times the area its lattice gives it is overloaded.
const OVERLOAD: Scalar = 2.0;
/// Two tips closer than this share of a spacing hold the same place twice. Wider apart is
/// the three a small part stands on, which placement puts there on purpose.
const CROWDED: Scalar = 0.2;
/// A trunk taller than this many of its own widths, with nothing to brace it to, sways on
/// the peel.
const SLENDER: Scalar = 30.0;
/// A bare patch smaller than this is a ledge a head already covers, square millimetres.
const MIN_BARE_MM2: Scalar = 1.0;
/// How finely a face is cut to share it between tips, millimetres: well inside a spacing.
const PIECE_MM: Scalar = 0.25;
/// How far below a place the layer that may carry it is looked for, millimetres, as
/// placement measures its reach.
const REFERENCE_RISE_MM: Scalar = 1.0;
/// How far off a face its outside is probed, millimetres: under a pixel, over rounding.
const BURIED_PROBE_MM: Scalar = 1e-3;

/// What one model's supports came to, as geometry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Shape {
    pub contacts: usize,
    pub standing: usize,
    pub trees: usize,
    /// Supports whose foot stands on the model rather than the plate: a scar of its own.
    pub on_model: usize,
    pub supports_ml: f32,
    pub model_ml: f32,
    /// Underside steeper than the profile's overhang angle, square millimetres.
    pub overhang_mm2: f32,
    /// Of that, what is further than a spacing from every standing tip.
    pub bare_mm2: f32,
    /// What all tips together leave on the surface, square millimetres, at the section the
    /// surface cuts through each.
    pub scar_mm2: f32,
    /// Tips on a face that looks up in the model's upright pose: the side meant to be seen.
    pub visible: usize,
    pub thin_tips: usize,
    pub crowded: usize,
    pub slender: usize,
    /// Overhang carried per standing tip, square millimetres.
    pub load_p50_mm2: f32,
    pub load_p90_mm2: f32,
    pub load_max_mm2: f32,
    /// What the lattice gives one tip, square millimetres: the spacing squared.
    pub design_mm2: f32,
    /// The section of one tip where it meets the part, square millimetres.
    pub tip_mm2: f32,
    pub overloaded: usize,
    pub findings: Vec<Finding>,
}

impl Shape {
    /// Underside the busiest tips carry for each square millimetre of their own section:
    /// what the peel pulls through a tip, as a share of what the tip is.
    pub fn carry_ratio(&self) -> f32 {
        if self.tip_mm2 > 0.0 {
            self.load_p90_mm2 / self.tip_mm2
        } else {
            0.0
        }
    }
}

/// One place worth looking at, plate millimetres.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Finding {
    pub kind: Kind,
    pub at: [f32; 3],
    /// What the kind measures there: area, thickness, load or ratio.
    pub value: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Hanging,
    Bare,
    Overloaded,
    ThinTip,
    Visible,
    Crowded,
    Slender,
    OnModel,
}

/// Reads `supported` against `profile`; `upright` turns the printed pose back to the one
/// the model was drawn in.
pub fn read(supported: &Supported, profile: &SupportProfile, upright: Quat) -> Shape {
    let spacing = profile.contact_spacing_mm / profile.density.max(1e-3).sqrt();
    let standing = standing(supported);
    let tips: Vec<Vec3> = standing.iter().map(|&at| supported.contacts[at]).collect();
    let bvh = Bvh::build(&supported.model);
    let mut shape = Shape {
        contacts: supported.contacts.len(),
        standing: tips.len(),
        trees: supported.trees.len(),
        supports_ml: signed_volume(&supported.supports).abs() / 1000.0,
        model_ml: signed_volume(&supported.model).abs() / 1000.0,
        scar_mm2: tips.len() as Scalar * std::f32::consts::PI * surface_radius_mm(profile).powi(2),
        design_mm2: spacing * spacing,
        tip_mm2: std::f32::consts::PI * surface_radius_mm(profile).powi(2),
        ..Shape::default()
    };
    hanging(&mut shape, supported, &standing);
    underside(&mut shape, supported, profile, &tips, spacing);
    tips_on_surface(&mut shape, &supported.model, &bvh, &tips, upright);
    crowding(&mut shape, &tips, spacing);
    trees(&mut shape, supported, profile);
    shape
}

/// How wide a tip is where it crosses the part's surface, millimetres. A plane tip is the
/// whole cone moved its bite in, so the surface cuts it wider than its contact disc
/// (ADR 0243); that section is what the peel pulls through and what is left as a scar.
pub fn surface_radius_mm(profile: &SupportProfile) -> Scalar {
    let contact = profile.contact_radius_mm();
    match profile.tip.shape {
        ContactShape::Plane => {
            let top = profile.top_length_mm().max(1e-3);
            let bite = profile.contact_depth_mm().min(top);
            contact + (profile.pillar_radius_mm() - contact) * bite / top
        }
        ContactShape::Sphere => contact,
    }
}

/// Indices of the contacts some tree carries.
fn standing(supported: &Supported) -> Vec<usize> {
    let mut held: Vec<usize> = supported
        .trees
        .iter()
        .flat_map(|tree| tree.points())
        .collect();
    held.sort_unstable();
    held.dedup();
    held
}

fn hanging(shape: &mut Shape, supported: &Supported, standing: &[usize]) {
    for (at, contact) in supported.contacts.iter().enumerate() {
        if standing.binary_search(&at).is_err() {
            shape.findings.push(finding(Kind::Hanging, *contact, 0.0));
        }
    }
}

fn finding(kind: Kind, at: Vec3, value: Scalar) -> Finding {
    Finding {
        kind,
        at: at.to_array(),
        value,
    }
}

/// Tips bucketed by a grid of `cell` millimetres, so the nearest is asked of 27 cells.
struct Grid {
    cell: Scalar,
    buckets: FastMap<[i32; 3], Vec<usize>>,
}

impl Grid {
    fn new(points: &[Vec3], cell: Scalar) -> Self {
        let mut buckets: FastMap<[i32; 3], Vec<usize>> = FastMap::default();
        for (at, point) in points.iter().enumerate() {
            buckets.entry(Self::key(*point, cell)).or_default().push(at);
        }
        Self { cell, buckets }
    }

    fn key(point: Vec3, cell: Scalar) -> [i32; 3] {
        (point / cell).floor().as_ivec3().to_array()
    }

    /// The nearest of `points` to `at` within one cell, and how far it is.
    fn nearest(&self, points: &[Vec3], at: Vec3) -> Option<(usize, Scalar)> {
        let [x, y, z] = Self::key(at, self.cell);
        let mut best: Option<(usize, Scalar)> = None;
        for key in (-1..=1).flat_map(|dx| {
            (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| [x + dx, y + dy, z + dz]))
        }) {
            for &index in self.buckets.get(&key).into_iter().flatten() {
                let distance = points[index].distance(at);
                if distance <= self.cell && best.is_none_or(|(_, d)| distance < d) {
                    best = Some((index, distance));
                }
            }
        }
        best
    }
}

/// The underside each tip carries, and the patches no tip reaches.
fn underside(
    shape: &mut Shape,
    supported: &Supported,
    profile: &SupportProfile,
    tips: &[Vec3],
    spacing: Scalar,
) {
    let needs = -profile.max_overhang_deg.to_radians().sin();
    let grid = Grid::new(tips, spacing);
    let closed = diagnose(&supported.model).boundary_edges == 0;
    let bvh = Bvh::build(&supported.model);
    let mut load = vec![0.0; tips.len()];
    let mut bare: FastMap<[i32; 3], (Scalar, Vec3)> = FastMap::default();
    for triangle in supported.model.triangles() {
        let normal = triangle.normal_unnormalized().normalize_or_zero();
        if normal.z >= needs {
            continue;
        }
        let area = triangle.area();
        let centre = (triangle.a + triangle.b + triangle.c) / 3.0;
        if closed && buried(&bvh, centre + normal * BURIED_PROBE_MM) {
            continue;
        }
        shape.overhang_mm2 += area;
        for (point, piece) in pieces(&triangle) {
            match grid.nearest(tips, point) {
                Some((tip, _)) => load[tip] += piece * -normal.z,
                None if carried_below(&bvh, point, normal, profile.max_overhang_deg) => {}
                None => {
                    let patch = bare.entry(Grid::key(point, spacing)).or_default();
                    patch.0 += piece;
                    patch.1 = point;
                }
            }
        }
    }
    for (area, centre) in bare.into_values() {
        shape.bare_mm2 += area;
        if area >= MIN_BARE_MM2 {
            shape.findings.push(finding(Kind::Bare, centre, area));
        }
    }
    loads(shape, tips, &load);
}

/// Whether material under `point` within a millimetre of rise, leaning no further than
/// `max_overhang_deg`, holds it: the reach placement itself allows (docs/design/supports.md).
fn carried_below(bvh: &Bvh, point: Vec3, normal: Vec3, max_overhang_deg: Scalar) -> bool {
    let lean = max_overhang_deg.to_radians();
    let from = point + normal * BURIED_PROBE_MM;
    let reach = REFERENCE_RISE_MM / lean.cos();
    let down = (0..8).map(|k| {
        let turn = k as Scalar * std::f32::consts::FRAC_PI_4;
        Vec3::new(
            lean.sin() * turn.cos(),
            lean.sin() * turn.sin(),
            -lean.cos(),
        )
    });
    std::iter::once(Vec3::NEG_Z).chain(down).any(|direction| {
        bvh.raycast(&Ray::new(from, direction))
            .is_some_and(|hit| hit.t <= reach)
    })
}

/// `triangle` cut into pieces no wider than `PIECE_MM`, as the middle and area of each: a
/// face of a coarse mesh is shared by many tips, not carried by the nearest.
fn pieces(triangle: &Triangle) -> Vec<(Vec3, Scalar)> {
    let longest = [
        triangle.b - triangle.a,
        triangle.c - triangle.b,
        triangle.a - triangle.c,
    ]
    .map(Vec3::length)
    .into_iter()
    .fold(0.0, Scalar::max);
    let cuts = ((longest / PIECE_MM).ceil() as usize).clamp(1, 256);
    let area = triangle.area() / (cuts * cuts) as Scalar;
    let step = 1.0 / cuts as Scalar;
    let at = |u: Scalar, v: Scalar| {
        triangle.a + (triangle.b - triangle.a) * u + (triangle.c - triangle.a) * v
    };
    let mut pieces = Vec::with_capacity(cuts * cuts);
    for i in 0..cuts {
        for j in 0..cuts - i {
            let (u, v) = (i as Scalar * step, j as Scalar * step);
            pieces.push((at(u + step / 3.0, v + step / 3.0), area));
            if i + j + 1 < cuts {
                pieces.push((at(u + 2.0 * step / 3.0, v + 2.0 * step / 3.0), area));
            }
        }
    }
    pieces
}

/// Whether `point` is inside the solid, by the parity of the surfaces a ray from it
/// crosses: a face of one shell inside another is no underside of the union.
fn buried(bvh: &Bvh, point: Vec3) -> bool {
    let direction = Vec3::new(0.37, 0.29, 0.88).normalize();
    let mut from = point;
    let mut crossings = 0;
    while let Some(hit) = bvh.raycast(&Ray::new(from, direction)) {
        crossings += 1;
        from += direction * (hit.t + BURIED_PROBE_MM);
        if crossings > 512 {
            return false;
        }
    }
    crossings % 2 == 1
}

fn loads(shape: &mut Shape, tips: &[Vec3], load: &[Scalar]) {
    let mut sorted = load.to_vec();
    sorted.sort_by(Scalar::total_cmp);
    let at = |share: Scalar| sorted.get(((sorted.len() as Scalar - 1.0) * share).round() as usize);
    shape.load_p50_mm2 = at(0.5).copied().unwrap_or(0.0);
    shape.load_p90_mm2 = at(0.9).copied().unwrap_or(0.0);
    shape.load_max_mm2 = sorted.last().copied().unwrap_or(0.0);
    for (tip, carried) in tips.iter().zip(load) {
        if *carried > OVERLOAD * shape.design_mm2 {
            shape.overloaded += 1;
            shape
                .findings
                .push(finding(Kind::Overloaded, *tip, *carried));
        }
    }
}

/// Tips on details too thin to lose one, and tips on the side meant to be seen.
fn tips_on_surface(shape: &mut Shape, model: &Mesh, bvh: &Bvh, tips: &[Vec3], upright: Quat) {
    let back = upright.inverse();
    for tip in tips {
        let Some(closest) = bvh.closest(*tip) else {
            continue;
        };
        let Some(triangle) = model.triangle(closest.face) else {
            continue;
        };
        let normal = triangle.normal_unnormalized().normalize_or_zero();
        if (back * normal).z > LOOKS_UP {
            shape.visible += 1;
            shape.findings.push(finding(Kind::Visible, *tip, 0.0));
        }
        if let Some(thickness) = thickness(bvh, closest.point, normal)
            && thickness < THIN_DETAIL_MM
        {
            shape.thin_tips += 1;
            shape.findings.push(finding(Kind::ThinTip, *tip, thickness));
        }
    }
}

/// How much material stands behind `surface` along `-normal`, millimetres, or `None` where
/// the ray never comes out, as through a hole in an open scan.
fn thickness(bvh: &Bvh, surface: Vec3, normal: Vec3) -> Option<Scalar> {
    let inward = -normal;
    let from = surface + inward * 1e-3;
    bvh.raycast(&Ray::new(from, inward)).map(|hit| hit.t + 1e-3)
}

fn crowding(shape: &mut Shape, tips: &[Vec3], spacing: Scalar) {
    let reach = CROWDED * spacing;
    for (at, tip) in tips.iter().enumerate() {
        let near = tips
            .iter()
            .enumerate()
            .any(|(other, point)| other != at && point.distance(*tip) < reach);
        if near {
            shape.crowded += 1;
            shape.findings.push(finding(Kind::Crowded, *tip, 0.0));
        }
    }
}

fn trees(shape: &mut Shape, supported: &Supported, profile: &SupportProfile) {
    let roots: Vec<Vec3> = supported
        .trees
        .iter()
        .map(|tree| tree.landing().base)
        .collect();
    for (at, tree) in supported.trees.iter().enumerate() {
        let landing = tree.landing();
        if landing.on_model && !landing.on_support {
            shape.on_model += 1;
            shape
                .findings
                .push(finding(Kind::OnModel, landing.base, 0.0));
        }
        let top = tree
            .nodes()
            .iter()
            .map(|node| node.position.z)
            .fold(Scalar::MIN, Scalar::max);
        let width = 2.0 * tree.root().radius_mm;
        let ratio = (top - landing.base.z) / width.max(1e-3);
        let braced = profile.bracing.enabled
            && roots.iter().enumerate().any(|(other, root)| {
                other != at
                    && root.truncate().distance(landing.base.truncate())
                        <= profile.bracing.max_spacing_mm
            });
        if !landing.on_model && !braced && ratio > SLENDER {
            shape.slender += 1;
            shape
                .findings
                .push(finding(Kind::Slender, landing.base, ratio));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plane_tip_crosses_the_surface_wider_than_its_contact() {
        let mut profile = SupportProfile::medium();
        profile.tip.shape = ContactShape::Plane;
        profile.tip.contact_diameter_mm = 0.4;
        profile.tip.contact_depth_mm = 0.5;
        profile.top.length_mm = 2.0;
        profile.middle.diameter_mm = 1.2;
        let radius = surface_radius_mm(&profile);
        // The cone runs 0.2 to 0.6 mm over 2 mm and is sunk a quarter of that.
        assert!(
            (radius - 0.3).abs() < 1e-6,
            "a quarter of the way up the cone is 0.3 mm, got {radius}"
        );
    }

    #[test]
    fn a_face_cut_into_pieces_keeps_its_area() {
        let triangle = Triangle::new(
            Vec3::ZERO,
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
        );
        let pieces = pieces(&triangle);
        let area: Scalar = pieces.iter().map(|(_, area)| area).sum();
        assert!(
            (area - 6.0).abs() < 1e-4,
            "a 3-4-5 right triangle holds 6 mm², got {area}"
        );
        assert!(
            pieces.len() > 100,
            "a 5 mm edge is cut at a quarter millimetre"
        );
    }

    #[test]
    fn a_point_inside_a_closed_box_is_buried_and_one_outside_is_not() {
        let cube = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let bvh = Bvh::build(&cube);
        assert!(buried(&bvh, Vec3::splat(5.0)));
        assert!(!buried(&bvh, Vec3::new(5.0, 5.0, -1.0)));
    }
}

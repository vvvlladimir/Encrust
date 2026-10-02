use core_geometry::{Aabb, Mesh, Scalar, Vec3, glam::IVec3};
use rayon::prelude::*;

use serde::{Deserialize, Serialize};

use crate::error::VolumeError;
use crate::grid::TILE;
use crate::sdf::Sdf;

/// The coarsest and finest a lattice chunks its own walls, as a fraction of one cell.
///
/// A wall follows the cavity by standing a box under every chunk of itself, so this is
/// what `precision` buys: how closely the lattice's ends trace the shell.
const COARSE_CHUNK: Scalar = 4.0;
const FINE_CHUNK: Scalar = 16.0;

/// Which lattice stands in a cavity.
///
/// All three are open networks rather than closed cells, because resin has to be able to
/// leave; see `docs/design/hollowing.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InfillPattern {
    /// Hexagonal tubes standing on the plate. The stiffest of the three for its weight,
    /// and what a resin slicer usually means by a honeycomb.
    #[default]
    Hive,
    /// Square tubes standing on the plate, on the lattice's own x and y planes.
    Grid,
    /// Struts along all three axes, meeting at the corners of a cubic lattice. The only
    /// one of the three that carries a load sideways.
    Scaffold,
}

impl InfillPattern {
    /// The patterns a picker offers, in the order it draws them.
    pub const ALL: [Self; 3] = [Self::Hive, Self::Grid, Self::Scaffold];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hive => "Hive",
            Self::Grid => "Grid",
            Self::Scaffold => "Scaffold",
        }
    }

    /// Wall thickness a cell of `size_mm` needs to come to `density`, in millimetres.
    ///
    /// Each is the inverse of the pattern's own solid fraction, so density is the number
    /// the user states and the thickness is what follows from it.
    fn thickness_mm(self, size_mm: Scalar, density: Scalar) -> Scalar {
        let shrink = 1.0 - (1.0 - density).sqrt();
        match self {
            // A square tube of pitch `c` and wall `t` is solid over `1 - (1 - t/c)^2`.
            Self::Grid => size_mm * shrink,
            // The same, measured on a hexagon's inradius rather than half a pitch.
            Self::Hive => size_mm * (3.0 as Scalar).sqrt() / 2.0 * shrink,
            // Three square struts through a cell are solid over about `3 t^2 / c^2`.
            Self::Scaffold => size_mm * (density / 3.0).sqrt(),
        }
    }
}

/// What fills the cavity, how big one cell of it is and how much of the cavity it takes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InfillSettings {
    pub pattern: InfillPattern,
    /// Side of one cell, in millimetres. For [`InfillPattern::Hive`] it is the diameter
    /// of a hexagon's circumscribed circle, which is how every resin slicer states it.
    pub size_mm: Scalar,
    /// How much of the cavity the lattice fills, from 0 to 1. The wall thickness follows
    /// from this and the cell size rather than being a number of its own.
    pub density: Scalar,
}

impl Default for InfillSettings {
    fn default() -> Self {
        Self {
            pattern: InfillPattern::default(),
            size_mm: 5.0,
            density: 0.15,
        }
    }
}

impl InfillSettings {
    /// The wall or strut thickness this cell and density come to, in millimetres.
    pub fn thickness_mm(&self) -> Scalar {
        self.pattern.thickness_mm(self.size_mm, self.density)
    }

    fn validate(&self) -> Result<(), VolumeError> {
        if !self.size_mm.is_finite() || self.size_mm <= 0.0 {
            return Err(VolumeError::BadCell(self.size_mm));
        }
        if !self.density.is_finite() || self.density <= 0.0 || self.density >= 1.0 {
            return Err(VolumeError::BadDensity(self.density));
        }
        Ok(())
    }
}

/// The lattice standing in `cavity`, as a mesh of its own.
///
/// Every piece is a closed box and the boxes overlap rather than being welded, because the
/// non-zero winding rule the rasteriser fills by already unions them. Nothing is
/// voxelised: the cost follows the number of cells, not the lattice the cavity was cut on.
pub(crate) fn lattice(
    cavity: &Sdf,
    settings: &InfillSettings,
    precision: Scalar,
    bond_mm: Scalar,
) -> Result<Mesh, VolumeError> {
    settings.validate()?;
    let Some(bounds) = cavity_bounds(cavity) else {
        return Ok(Mesh::default());
    };

    let thickness_mm = settings.thickness_mm();
    let step_mm = cavity.grid().voxel_mm;
    let chunk_mm = settings.size_mm / (COARSE_CHUNK + (FINE_CHUNK - COARSE_CHUNK) * precision);

    let shape = Shape {
        thickness: thickness_mm,
        step: step_mm,
        bond: bond_mm,
    };
    let parts: Vec<Mesh> = match settings.pattern {
        InfillPattern::Hive | InfillPattern::Grid => footprint(settings, &bounds)
            .into_par_iter()
            .map(|(from, to)| {
                let mut mesh = Mesh::default();
                walls(cavity, from, to, chunk_mm, &bounds, &shape, &mut mesh);
                mesh
            })
            .collect(),
        InfillPattern::Scaffold => struts(settings.size_mm, &bounds)
            .into_par_iter()
            .map(|(from, to)| {
                let mut mesh = Mesh::default();
                pieces(cavity, from, to, &shape, &mut mesh);
                mesh
            })
            .collect(),
    };
    Ok(joined(parts))
}

/// One mesh out of many, with each part's faces moved onto its own vertices.
fn joined(parts: Vec<Mesh>) -> Mesh {
    let mut whole = Mesh::default();
    whole
        .vertices
        .reserve(parts.iter().map(|part| part.vertices.len()).sum());
    whole
        .faces
        .reserve(parts.iter().map(|part| part.faces.len()).sum());
    for part in parts {
        let offset = whole.vertices.len() as u32;
        whole.vertices.extend_from_slice(&part.vertices);
        whole.faces.extend(
            part.faces
                .iter()
                .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
        );
    }
    whole
}

/// How thick a piece is, how finely the cavity under it is walked, and how far it may
/// reach past the cavity to meet the wall. All three in millimetres.
struct Shape {
    thickness: Scalar,
    step: Scalar,
    bond: Scalar,
}

/// The box the cavity's own tiles stand in.
fn cavity_bounds(cavity: &Sdf) -> Option<Aabb> {
    let grid = cavity.grid();
    let mut tiles = cavity.tile_keys();
    let first = tiles.next()?;
    let (low, high) = tiles.fold((first, first), |(low, high), tile| {
        (low.min(tile), high.max(tile))
    });
    Some(Aabb::new(
        grid.position(low * TILE),
        grid.position((high + IVec3::ONE) * TILE),
    ))
}

/// The two-dimensional pattern a vertical lattice is the extrusion of, as the segments of
/// its own walls, anchored on the global lattice so it does not move when a model does.
fn footprint(settings: &InfillSettings, bounds: &Aabb) -> Vec<(Vec3, Vec3)> {
    match settings.pattern {
        InfillPattern::Grid => grid_lines(settings.size_mm, bounds),
        _ => hive_edges(settings.size_mm, bounds),
    }
}

/// Walls on the lattice's own x and y planes, each spanning the bounds.
fn grid_lines(size_mm: Scalar, bounds: &Aabb) -> Vec<(Vec3, Vec3)> {
    let mut lines = Vec::new();
    for axis in 0..2 {
        for step in steps(size_mm, bounds.mins[axis], bounds.maxs[axis]) {
            let mut from = bounds.mins;
            let mut to = bounds.maxs;
            from[axis] = step;
            to[axis] = step;
            to.z = bounds.mins.z;
            lines.push((from, to));
        }
    }
    lines
}

/// The three edges each hexagon of a honeycomb owns, over the bounds.
///
/// A flat-topped hexagon of circumradius `r` tiles on a rectangle `1.5 r` across and
/// `sqrt(3) r` up, with every other column offset by half a row.
fn hive_edges(size_mm: Scalar, bounds: &Aabb) -> Vec<(Vec3, Vec3)> {
    let radius = size_mm / 2.0;
    let (across, up) = (1.5 * radius, (3.0 as Scalar).sqrt() * radius);
    let corner = |center: Vec3, step: usize| {
        let angle = std::f32::consts::FRAC_PI_3 * step as Scalar;
        center + Vec3::new(radius * angle.cos(), radius * angle.sin(), 0.0)
    };

    let mut edges = Vec::new();
    for (column, x) in steps(across, bounds.mins.x, bounds.maxs.x)
        .into_iter()
        .enumerate()
    {
        let offset = if column % 2 == 0 { 0.0 } else { up / 2.0 };
        for y in steps(up, bounds.mins.y - offset, bounds.maxs.y) {
            let center = Vec3::new(x, y + offset, bounds.mins.z);
            // Three of the six, so two neighbouring cells do not both draw the edge they
            // share.
            for step in 0..3 {
                edges.push((corner(center, step), corner(center, step + 1)));
            }
        }
    }
    edges
}

/// The struts of a cubic scaffold, each spanning the bounds along one axis.
fn struts(size_mm: Scalar, bounds: &Aabb) -> Vec<(Vec3, Vec3)> {
    let mut struts = Vec::new();
    for axis in 0..3 {
        let (one, other) = ((axis + 1) % 3, (axis + 2) % 3);
        for first in steps(size_mm, bounds.mins[one], bounds.maxs[one]) {
            for second in steps(size_mm, bounds.mins[other], bounds.maxs[other]) {
                let mut from = bounds.mins;
                let mut to = bounds.maxs;
                from[one] = first;
                to[one] = first;
                from[other] = second;
                to[other] = second;
                struts.push((from, to));
            }
        }
    }
    struts
}

/// Every multiple of `pitch` between `from` and `to`, on the global lattice.
fn steps(pitch: Scalar, from: Scalar, to: Scalar) -> Vec<Scalar> {
    let first = (from / pitch).floor() as i32;
    let last = (to / pitch).ceil() as i32;
    (first..=last).map(|step| step as Scalar * pitch).collect()
}

/// Stands a wall along the footprint segment `from`–`to`, as few boxes as its own span
/// allows.
///
/// The segment is walked in chunks and a chunk's span is intersected into the run before
/// it, so a box is only ever shorter than the cavity under it — longer would reach out
/// through the wall, where the winding count has nothing to cancel it. A run is only
/// broken where holding it would cost more than a lattice step of height, so a wall
/// crossing the middle of a cavity is one box and only its ends are chased in detail. A
/// box per chunk would put one contour per chunk on every layer the wall crosses, which
/// is what makes a slice stack of a dense lattice unaffordable.
fn walls(
    cavity: &Sdf,
    from: Vec3,
    to: Vec3,
    chunk_mm: Scalar,
    bounds: &Aabb,
    shape: &Shape,
    mesh: &mut Mesh,
) {
    let along = Vec3::new(to.x - from.x, to.y - from.y, 0.0);
    let length = along.length();
    if length <= 0.0 {
        return;
    }

    let direction = along / length;
    let side = Vec3::new(-direction.y, direction.x, 0.0);
    let chunks = (length / chunk_mm).ceil().max(1.0) as usize;
    let chunk = length / chunks as Scalar;

    let mut run: Option<(Scalar, Vec<(Scalar, Scalar)>)> = None;
    for index in 0..chunks {
        let start = from + direction * (index as Scalar * chunk);
        let at = [
            start,
            start + direction * (chunk / 2.0),
            start + direction * chunk,
        ];
        let here = shared_spans(cavity, at, bounds.mins.z, bounds.maxs.z, shape.step);

        let at_mm = index as Scalar * chunk;
        let held = run.take().and_then(|(began, spans)| {
            let merged = overlaps(&spans, &here);
            if keeps_its_height(&spans, &merged, shape.step) {
                return Some((began, merged));
            }
            flush(from, direction, side, began, at_mm, &spans, shape, mesh);
            None
        });
        run = held.or_else(|| (!here.is_empty()).then_some((at_mm, here)));
    }
    if let Some((began, spans)) = run {
        flush(from, direction, side, began, length, &spans, shape, mesh);
    }
}

/// Whether a run is still worth holding: merging must not have cost it more than a step
/// of height, or the wall would pull away from the shell it is meant to reach.
fn keeps_its_height(
    before: &[(Scalar, Scalar)],
    after: &[(Scalar, Scalar)],
    step_mm: Scalar,
) -> bool {
    if after.len() != before.len() {
        return false;
    }
    let height =
        |spans: &[(Scalar, Scalar)]| -> Scalar { spans.iter().map(|(low, high)| high - low).sum() };
    height(before) - height(after) <= step_mm
}

/// Stands one box per span over the stretch of wall from `began` to `ended`.
#[allow(clippy::too_many_arguments)]
fn flush(
    from: Vec3,
    direction: Vec3,
    side: Vec3,
    began: Scalar,
    ended: Scalar,
    spans: &[(Scalar, Scalar)],
    shape: &Shape,
    mesh: &mut Mesh,
) {
    let middle = from + direction * Scalar::midpoint(began, ended);
    for (bottom, top) in spans {
        let (bottom, top) = (bottom - shape.bond, top + shape.bond);
        mesh_box(
            Vec3::new(middle.x, middle.y, Scalar::midpoint(bottom, top)),
            direction * ((ended - began) / 2.0),
            side * (shape.thickness / 2.0),
            Vec3::new(0.0, 0.0, (top - bottom) / 2.0),
            mesh,
        );
    }
}

/// Stands a strut along `from`–`to`, one box per stretch of it inside the cavity.
fn pieces(cavity: &Sdf, from: Vec3, to: Vec3, shape: &Shape, mesh: &mut Mesh) {
    let along = to - from;
    let length = along.length();
    if length <= 0.0 {
        return;
    }
    let direction = along / length;
    let (side, up) = perpendiculars(direction);

    for (start, end) in spans_along(cavity, from, to, shape.step) {
        let (start, end) = (start - shape.bond, end + shape.bond);
        mesh_box(
            from + direction * Scalar::midpoint(start, end),
            direction * ((end - start) / 2.0),
            side * (shape.thickness / 2.0),
            up * (shape.thickness / 2.0),
            mesh,
        );
    }
}

/// The stretches of a vertical line that every one of `at` agrees is inside the cavity.
fn shared_spans(
    cavity: &Sdf,
    at: [Vec3; 3],
    bottom_mm: Scalar,
    top_mm: Scalar,
    step_mm: Scalar,
) -> Vec<(Scalar, Scalar)> {
    let column = |point: Vec3| {
        let low = Vec3::new(point.x, point.y, bottom_mm);
        let high = Vec3::new(point.x, point.y, top_mm);
        spans_along(cavity, low, high, step_mm)
            .into_iter()
            .map(|(start, end)| (bottom_mm + start, bottom_mm + end))
            .collect::<Vec<_>>()
    };

    let mut shared = column(at[0]);
    for point in &at[1..] {
        if shared.is_empty() {
            break;
        }
        shared = overlaps(&shared, &column(*point));
    }
    shared
}

/// The stretches two sets of spans have in common.
fn overlaps(left: &[(Scalar, Scalar)], right: &[(Scalar, Scalar)]) -> Vec<(Scalar, Scalar)> {
    let mut shared = Vec::new();
    for (one_low, one_high) in left {
        for (other_low, other_high) in right {
            let (low, high) = (one_low.max(*other_low), one_high.min(*other_high));
            if high > low {
                shared.push((low, high));
            }
        }
    }
    shared
}

/// How far along `from`–`to` the cavity is, in millimetres from `from`.
///
/// The field is sampled rather than meshed, which is why a lattice costs what its cells
/// cost and not what its own surface would.
fn spans_along(cavity: &Sdf, from: Vec3, to: Vec3, step_mm: Scalar) -> Vec<(Scalar, Scalar)> {
    let along = to - from;
    let length = along.length();
    if length <= 0.0 || step_mm <= 0.0 {
        return Vec::new();
    }
    let direction = along / length;
    let steps = (length / step_mm).ceil() as usize;

    let mut spans = Vec::new();
    let mut open: Option<Scalar> = None;
    let mut previous = (0.0, cavity.sample(from));
    if previous.1 < 0.0 {
        open = Some(0.0);
    }

    for step in 1..=steps {
        let at = (step as Scalar * step_mm).min(length);
        let value = cavity.sample(from + direction * at);
        if (previous.1 < 0.0) != (value < 0.0) {
            let cut = previous.0 + (at - previous.0) * crossing(previous.1, value);
            match open.take() {
                Some(start) => spans.push((start, cut)),
                None => open = Some(cut),
            }
        }
        previous = (at, value);
    }
    if let Some(start) = open {
        spans.push((start, length));
    }
    spans
}

/// Where between two sampled values the field changes sign.
fn crossing(near: Scalar, far: Scalar) -> Scalar {
    let span = near - far;
    if span.abs() > Scalar::EPSILON {
        (near / span).clamp(0.0, 1.0)
    } else {
        0.5
    }
}

/// Two unit vectors at right angles to `direction` and to each other.
fn perpendiculars(direction: Vec3) -> (Vec3, Vec3) {
    let seed = if direction.z.abs() < 0.9 {
        Vec3::Z
    } else {
        Vec3::X
    };
    let side = direction.cross(seed).normalize();
    (side, direction.cross(side))
}

/// A box's six faces, each as its four corners wound anticlockwise seen from outside.
const BOX_FACES: [[u32; 4]; 6] = [
    [0, 2, 3, 1],
    [4, 5, 7, 6],
    [0, 1, 5, 4],
    [2, 6, 7, 3],
    [0, 4, 6, 2],
    [1, 3, 7, 5],
];

/// Appends one closed box, given its centre and three half-extent vectors.
///
/// Boxes overlap rather than weld: the fill rule unions them, which is the same trick a
/// support's struts are meshed with.
fn mesh_box(center: Vec3, u: Vec3, v: Vec3, w: Vec3, mesh: &mut Mesh) {
    let first = mesh.vertices.len() as u32;
    for corner in 0..8u32 {
        let sign = |bit: u32| if corner & (1 << bit) == 0 { -1.0 } else { 1.0 };
        mesh.vertices
            .push(center + u * sign(0) + v * sign(1) + w * sign(2));
    }

    for face in BOX_FACES {
        mesh.faces
            .push([first + face[0], first + face[1], first + face[2]]);
        mesh.faces
            .push([first + face[0], first + face[2], first + face[3]]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{VoxelGrid, tile_of};
    use core_geometry::{diagnose, signed_volume};
    use std::collections::HashSet;

    fn hive(size_mm: Scalar, density: Scalar) -> InfillSettings {
        InfillSettings {
            pattern: InfillPattern::Hive,
            size_mm,
            density,
        }
    }

    /// A cavity of `radius` centred on the origin, on a 0.5 mm lattice.
    fn ball_cavity(radius: Scalar) -> Sdf {
        let grid = VoxelGrid::new(0.5);
        let reach = (radius / 0.5).ceil() as i32 + 8;
        let first = tile_of(IVec3::splat(-reach));
        let last = tile_of(IVec3::splat(reach));
        let mut candidates = HashSet::new();
        for z in first.z..=last.z {
            for y in first.y..=last.y {
                for x in first.x..=last.x {
                    candidates.insert(IVec3::new(x, y, z));
                }
            }
        }
        crate::csg::assemble(grid, 1.5, candidates, |voxel| {
            grid.position(voxel).length() - radius
        })
    }

    fn volume_of(mesh: &Mesh) -> Scalar {
        signed_volume(mesh)
    }

    #[test]
    fn a_density_is_roughly_the_fraction_of_the_cavity_the_lattice_fills() {
        // The mesh overlaps itself at every junction, so its own volume over-reads; the
        // ratio is what has to be about right.
        let radius = 20.0;
        let cavity = ball_cavity(radius);
        let ball = 4.0 / 3.0 * std::f32::consts::PI * radius * radius * radius;

        for density in [0.1, 0.25] {
            let mesh = lattice(&cavity, &hive(5.0, density), 0.5, 0.0).expect("lattice");
            let filled = volume_of(&mesh) / ball;
            assert!(
                (filled - density).abs() < density * 0.5,
                "a {density} lattice filled {filled} of the cavity"
            );
        }
    }

    #[test]
    fn every_piece_of_a_lattice_is_a_closed_box() {
        let mesh = lattice(&ball_cavity(12.0), &hive(5.0, 0.15), 0.5, 0.0).expect("lattice");
        let report = diagnose(&mesh);
        assert_eq!(report.boundary_edges, 0, "{report:?}");
        assert_eq!(
            report.faces % 12,
            0,
            "a box is twelve faces, got {report:?}"
        );
        assert!(volume_of(&mesh) > 0.0, "the boxes are wound inside out");
    }

    #[test]
    fn a_lattice_stays_inside_the_cavity_it_stands_in() {
        // Nothing may reach past the cavity by more than the bond it was given: outside
        // the model a box would be a contour with nothing around it, which the non-zero
        // winding rule fills as material.
        let radius = 12.0;
        let bond_mm = 0.5;
        let cavity = ball_cavity(radius);
        let mesh = lattice(&cavity, &hive(4.0, 0.2), 1.0, bond_mm).expect("lattice");

        assert!(!mesh.is_empty(), "the ball is big enough to hold a hive");
        for vertex in &mesh.vertices {
            let out = vertex.length() - radius;
            assert!(
                out <= bond_mm * (3.0 as Scalar).sqrt(),
                "a box corner stands {out} mm outside a cavity it may leave by {bond_mm}"
            );
        }
    }

    #[test]
    fn a_vertical_lattice_leaves_its_channels_open() {
        // A tube standing on the plate drains; a cell closed over the top does not. Every
        // face of a hive or a grid is either upright or flat, and a box's flat faces are
        // its own ends, never a lid across the channel beside it.
        for pattern in [InfillPattern::Hive, InfillPattern::Grid] {
            let settings = InfillSettings {
                pattern,
                size_mm: 5.0,
                density: 0.2,
            };
            let mesh = lattice(&ball_cavity(12.0), &settings, 0.5, 0.0).expect("lattice");
            assert!(!mesh.is_empty());
            for face in mesh.triangles() {
                let normal = (face.b - face.a).cross(face.c - face.a).normalize();
                assert!(
                    normal.z.abs() < 1e-3 || normal.z.abs() > 1.0 - 1e-3,
                    "{} has a face leaning across its own channel",
                    pattern.label()
                );
            }
        }
    }

    #[test]
    fn a_thicker_wall_is_what_a_denser_lattice_asks_for() {
        let thin = hive(5.0, 0.1).thickness_mm();
        let thick = hive(5.0, 0.3).thickness_mm();
        assert!(thick > thin, "{thick} is not thicker than {thin}");
        // A grid of 20% over a 5 mm cell leaves a 4.47 mm channel, so its wall is 0.53 mm.
        let grid = InfillSettings {
            pattern: InfillPattern::Grid,
            size_mm: 5.0,
            density: 0.2,
        };
        assert!((grid.thickness_mm() - 0.528).abs() < 0.01);
    }

    #[test]
    fn a_cell_of_no_size_is_refused() {
        assert_eq!(hive(0.0, 0.2).validate(), Err(VolumeError::BadCell(0.0)));
    }

    #[test]
    fn a_density_outside_nought_to_one_is_refused() {
        assert_eq!(hive(5.0, 0.0).validate(), Err(VolumeError::BadDensity(0.0)));
        assert_eq!(hive(5.0, 1.0).validate(), Err(VolumeError::BadDensity(1.0)));
    }

    #[test]
    fn an_empty_cavity_holds_no_lattice() {
        let empty = crate::csg::assemble(VoxelGrid::new(0.5), 1.5, HashSet::new(), |_| 1.0);
        let mesh = lattice(&empty, &hive(5.0, 0.2), 0.5, 0.0).expect("lattice");
        assert!(mesh.is_empty());
    }
}

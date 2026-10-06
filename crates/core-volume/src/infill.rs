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

/// How high the opening a hive or grid wall leaves over the floor of the cavity and under
/// its ceiling is, in millimetres: enough for resin to run from cell to cell towards a hole.
const OPENING_MM: Scalar = 1.5;

/// How far either side of a junction a wall still stands the whole height of the cavity,
/// as a fraction of the cell: a quarter, so the opening is the middle half of each side and
/// the bridge over it is short enough to print.
const POST_REACH: Scalar = 0.25;

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
        opening: Opening::Full,
    };
    let parts: Vec<Mesh> = match settings.pattern {
        InfillPattern::Hive | InfillPattern::Grid => footprint(settings, &bounds)
            .into_par_iter()
            .map(|wall| {
                let mut mesh = Mesh::default();
                opened_wall(cavity, &wall, chunk_mm, &bounds, &shape, &mut mesh);
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

/// How thick a piece is, how finely the cavity under it is walked, how far it may reach
/// past the cavity to meet the wall — all three in millimetres — and what it leaves of the
/// opening at the cavity's floor and ceiling.
#[derive(Clone, Copy)]
struct Shape {
    thickness: Scalar,
    step: Scalar,
    bond: Scalar,
    opening: Opening,
}

/// What a piece of wall does with the opening a cell needs over the floor of the cavity and
/// under its ceiling.
#[derive(Clone, Copy)]
enum Opening {
    /// Nothing: the piece is the whole height of the cavity, as a scaffold strut is.
    Full,
    /// The piece stops that far short of both, which is the way from one cell to the next.
    Clear(Scalar),
    /// Those two stretches alone, which is what carries a post to the shell above and
    /// below the open wall.
    Caps(Scalar),
}

/// The line one wall stands along, and the way across it.
struct Line {
    from: Vec3,
    direction: Vec3,
    side: Vec3,
}

impl Line {
    fn at(&self, along_mm: Scalar) -> Vec3 {
        self.from + self.direction * along_mm
    }
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

/// One wall of the footprint: the segment it stands on, where along that segment a post
/// reaches the shell, and how far either side of a post it does.
struct Wall {
    from: Vec3,
    to: Vec3,
    /// Millimetres along the segment from `from`, which is where a wall across this one
    /// meets it.
    posts: Vec<Scalar>,
    reach_mm: Scalar,
}

/// The two-dimensional pattern a vertical lattice is the extrusion of, as the segments of
/// its own walls, anchored on the global lattice so it does not move when a model does.
fn footprint(settings: &InfillSettings, bounds: &Aabb) -> Vec<Wall> {
    match settings.pattern {
        InfillPattern::Grid => grid_lines(settings.size_mm, bounds),
        _ => hive_edges(settings.size_mm, bounds),
    }
}

/// Walls on the lattice's own x and y planes, each spanning the bounds, with a post where
/// every wall across it meets it.
fn grid_lines(size_mm: Scalar, bounds: &Aabb) -> Vec<Wall> {
    let mut lines = Vec::new();
    for axis in 0..2 {
        let across = 1 - axis;
        let posts: Vec<Scalar> = steps(size_mm, bounds.mins[across], bounds.maxs[across])
            .into_iter()
            .map(|corner| corner - bounds.mins[across])
            .collect();
        for step in steps(size_mm, bounds.mins[axis], bounds.maxs[axis]) {
            let mut from = bounds.mins;
            let mut to = bounds.maxs;
            from[axis] = step;
            to[axis] = step;
            to.z = bounds.mins.z;
            lines.push(Wall {
                from,
                to,
                posts: posts.clone(),
                reach_mm: POST_REACH * size_mm,
            });
        }
    }
    lines
}

/// The three edges each hexagon of a honeycomb owns, over the bounds.
///
/// A flat-topped hexagon of circumradius `r` tiles on a rectangle `1.5 r` across and
/// `sqrt(3) r` up, with every other column offset by half a row.
fn hive_edges(size_mm: Scalar, bounds: &Aabb) -> Vec<Wall> {
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
            // An edge is one side of a cell, so its posts are its own two ends.
            for step in 0..3 {
                let (from, to) = (corner(center, step), corner(center, step + 1));
                let length = (to - from).length();
                edges.push(Wall {
                    from,
                    to,
                    posts: vec![0.0, length],
                    reach_mm: POST_REACH * length,
                });
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

/// Stands a wall along the segment of `wall`, open over the floor of the cavity and under
/// its ceiling everywhere but at its posts, so that no cell is closed off from the next.
///
/// The open wall is one walk along the whole segment, so it still merges into as few boxes
/// as the cavity allows (ADR 0060); only the posts, which carry it to the shell above and
/// below, are stood stretch by stretch. See ADR 0189.
fn opened_wall(
    cavity: &Sdf,
    wall: &Wall,
    chunk_mm: Scalar,
    bounds: &Aabb,
    shape: &Shape,
    mesh: &mut Mesh,
) {
    let length = (wall.to - wall.from).length();
    if length <= 0.0 {
        return;
    }
    let open = Shape {
        opening: Opening::Clear(OPENING_MM),
        ..*shape
    };
    walls(cavity, wall.from, wall.to, chunk_mm, bounds, &open, mesh);

    let capped = Shape {
        opening: Opening::Caps(OPENING_MM),
        ..*shape
    };
    let direction = (wall.to - wall.from) / length;
    for post in &wall.posts {
        let (from_mm, to_mm) = (
            (post - wall.reach_mm).max(0.0),
            (post + wall.reach_mm).min(length),
        );
        if to_mm <= from_mm {
            continue;
        }
        let (from, to) = (
            wall.from + direction * from_mm,
            wall.from + direction * to_mm,
        );
        walls(cavity, from, to, chunk_mm, bounds, &capped, mesh);
    }
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
/// is what makes a slice stack of a dense lattice unaffordable. Where the cavity ends
/// across the wall's way, the wall is carried on to meet it; see [`meet`].
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
    let line = Line {
        from,
        direction,
        side: Vec3::new(-direction.y, direction.x, 0.0),
    };
    let chunks = (length / chunk_mm).ceil().max(1.0) as usize;
    let chunk = length / chunks as Scalar;

    let mut run: Option<(Scalar, Vec<(Scalar, Scalar)>)> = None;
    let mut open_before = false;
    for index in 0..chunks {
        let at_mm = index as Scalar * chunk;
        let here = spans_over(cavity, &line, at_mm, at_mm + chunk, bounds, shape);
        match &run {
            Some((_, held)) if here.is_empty() => {
                meet(cavity, &line, (at_mm, chunk), held, bounds, shape, mesh);
            }
            None if !here.is_empty() && open_before => {
                meet(cavity, &line, (at_mm, -chunk), &here, bounds, shape, mesh);
            }
            // The segment starts inside and leaves the cavity within its first chunk.
            None if here.is_empty() && index == 0 => {
                let start = spans_over(cavity, &line, 0.0, 0.0, bounds, shape);
                if !start.is_empty() {
                    meet(cavity, &line, (0.0, chunk), &start, bounds, shape, mesh);
                }
            }
            _ => {}
        }
        open_before = here.is_empty();

        let held = run.take().and_then(|(began, spans)| {
            let merged = overlaps(&spans, &here);
            if keeps_its_height(&spans, &merged, shape.step) {
                return Some((began, merged));
            }
            flush(&line, began, at_mm, &spans, shape, mesh);
            None
        });
        run = held.or_else(|| (!here.is_empty()).then_some((at_mm, here)));
    }
    if let Some((began, spans)) = run {
        flush(&line, began, length, &spans, shape, mesh);
    }
}

/// The stretches of height the cavity holds all along `start`–`end` of `line`, as the
/// wall's own opening leaves them.
fn spans_over(
    cavity: &Sdf,
    line: &Line,
    start: Scalar,
    end: Scalar,
    bounds: &Aabb,
    shape: &Shape,
) -> Vec<(Scalar, Scalar)> {
    let at = [
        line.at(start),
        line.at(Scalar::midpoint(start, end)),
        line.at(end),
    ];
    let spans = shared_spans(cavity, at, bounds.mins.z, bounds.maxs.z, shape.step);
    match shape.opening {
        Opening::Full => spans,
        // The bond is added back when the box is stood up, so it is taken off here as
        // well, which leaves the opening measured from the cavity itself.
        Opening::Clear(mm) => spans
            .into_iter()
            .map(|(low, high)| (low + mm + shape.bond, high - mm - shape.bond))
            .filter(|(low, high)| high > low)
            .collect(),
        Opening::Caps(mm) => spans.into_iter().flat_map(|span| caps(span, mm)).collect(),
    }
}

/// The two ends of a span a post stands in, or the whole of one with no room to open: the
/// cap and the open wall above it overlap by the bond, so the two never leave a gap.
fn caps((low, high): (Scalar, Scalar), mm: Scalar) -> Vec<(Scalar, Scalar)> {
    if high - low <= 2.0 * mm {
        return vec![(low, high)];
    }
    vec![(low, low + mm), (high - mm, high)]
}

/// Carries a wall of `held` spans from `at_mm` across the last `reach_mm` of its way,
/// forward or back by its sign, to the side of the cavity, and fuses it in by the bond.
///
/// A chunk the cavity leaves part of the way across holds no span, so a wall walked in
/// chunks stops up to a chunk short of the side it runs into; along a vertical side that
/// gap is a corridor joining every cell along it. The stretch is walked again a lattice
/// step at a time, which follows a side curving away under or over it.
fn meet(
    cavity: &Sdf,
    line: &Line,
    (at_mm, reach_mm): (Scalar, Scalar),
    held: &[(Scalar, Scalar)],
    bounds: &Aabb,
    shape: &Shape,
    mesh: &mut Mesh,
) {
    let step = shape.step.copysign(reach_mm);
    let steps = (reach_mm / step).ceil() as usize;
    let mut reached = at_mm;
    let mut last = held.to_vec();
    for index in 0..steps {
        let near = at_mm + step * index as Scalar;
        let spans = spans_over(cavity, line, near, near + step, bounds, shape);
        if spans.is_empty() {
            break;
        }
        flush(
            line,
            near.min(near + step),
            near.max(near + step),
            &spans,
            shape,
            mesh,
        );
        reached = near + step;
        last = spans;
    }
    let Some(&(low, high)) = last.first() else {
        return;
    };
    // Past the last step that held, to where the side is at the middle of its height.
    let height = Scalar::midpoint(low, high);
    let start = Vec3::new(line.at(reached).x, line.at(reached).y, height);
    let across = line.direction * step.signum();
    let past = spans_along(cavity, start, start + across * shape.step, shape.step)
        .first()
        .filter(|(begin, _)| *begin <= 0.0)
        .map_or(0.0, |(_, end)| *end);
    let far = reached + (past + shape.bond).copysign(step);
    flush(line, reached.min(far), reached.max(far), &last, shape, mesh);
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
fn flush(
    line: &Line,
    began: Scalar,
    ended: Scalar,
    spans: &[(Scalar, Scalar)],
    shape: &Shape,
    mesh: &mut Mesh,
) {
    let middle = line.at(Scalar::midpoint(began, ended));
    for (bottom, top) in spans {
        let (bottom, top) = (bottom - shape.bond, top + shape.bond);
        mesh_box(
            Vec3::new(middle.x, middle.y, Scalar::midpoint(bottom, top)),
            line.direction * ((ended - began) / 2.0),
            line.side * (shape.thickness / 2.0),
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

    /// Each box of `mesh` as its lowest and highest corner: every box is upright, so that is
    /// all of it.
    fn boxes(mesh: &Mesh) -> Vec<(Vec3, Vec3)> {
        mesh.vertices
            .chunks(8)
            .map(|corners| {
                corners
                    .iter()
                    .fold((corners[0], corners[0]), |(low, high), corner| {
                        (low.min(*corner), high.max(*corner))
                    })
            })
            .collect()
    }

    fn covered(boxes: &[(Vec3, Vec3)], point: Vec3) -> bool {
        boxes
            .iter()
            .any(|(low, high)| point.cmpge(*low).all() && point.cmple(*high).all())
    }

    #[test]
    fn a_hive_wall_is_open_over_the_floor_in_the_middle_and_stands_on_it_at_its_ends() {
        let radius = 12.0;
        let cavity = ball_cavity(radius);
        let settings = hive(5.0, 0.2);
        let mesh = lattice(&cavity, &settings, 0.5, 0.0).expect("lattice");
        let boxes = boxes(&mesh);
        let bounds = cavity_bounds(&cavity).expect("the ball has tiles");

        let mut checked = 0;
        for Wall { from, to, .. } in hive_edges(settings.size_mm, &bounds) {
            let middle = from.lerp(to, 0.5).truncate();
            if middle.length() > radius / 2.0 {
                continue;
            }
            // The floor of a ball under (x, y) is where its lower half stands.
            let low = |at: Vec3| {
                let floor = -(radius * radius - at.truncate().length_squared()).sqrt();
                Vec3::new(at.x, at.y, floor + OPENING_MM / 2.0)
            };
            assert!(
                !covered(&boxes, low(from.lerp(to, 0.5))),
                "the middle of a wall leaves the floor open to the next cell"
            );
            assert!(
                covered(&boxes, low(from.lerp(to, 0.1))),
                "and its ends still stand on it"
            );
            checked += 1;
        }
        assert!(
            checked > 3,
            "the middle of the ball holds walls, got {checked}"
        );
    }

    #[test]
    fn a_grid_wall_leaves_the_floor_open_between_its_posts_and_still_crosses_in_one_box() {
        let radius = 12.0;
        let cell = 4.0;
        let cavity = ball_cavity(radius);
        let settings = InfillSettings {
            pattern: InfillPattern::Grid,
            size_mm: cell,
            density: 0.2,
        };
        let mesh = lattice(&cavity, &settings, 0.5, 0.0).expect("lattice");
        let boxes = boxes(&mesh);

        // A hair into the opening over the floor of the ball, which under (x, y) is where
        // its lower half stands. The line y = 0 is one of the lattice's own, and a post
        // stands on it wherever x is a multiple of the cell.
        let over_the_floor = |x: Scalar| {
            let floor = -(radius * radius - x * x).sqrt();
            Vec3::new(x, 0.0, floor + OPENING_MM / 2.0)
        };
        assert!(
            covered(&boxes, over_the_floor(cell)),
            "a post carries the wall to the floor"
        );
        assert!(
            !covered(&boxes, over_the_floor(cell / 2.0)),
            "and between two posts the resin runs under the wall to the next cell"
        );

        let widest = boxes
            .iter()
            .map(|(low, high)| (high.x - low.x).max(high.y - low.y))
            .fold(0.0, Scalar::max);
        assert!(
            widest > 3.0 * cell,
            "the open wall is one walk, so it still merges over several cells; widest box \
             {widest} mm against a {cell} mm cell"
        );
    }

    #[test]
    fn a_hive_wall_running_into_the_side_of_the_cavity_meets_it() {
        // Walked in chunks, a wall used to stop up to a chunk short of the side, leaving a
        // corridor round the equator that joined every cell along it.
        let radius = 12.0;
        let cavity = ball_cavity(radius);
        let settings = hive(5.0, 0.2);
        let mesh = lattice(&cavity, &settings, 0.5, 1.0).expect("lattice");
        let boxes = boxes(&mesh);
        let bounds = cavity_bounds(&cavity).expect("the ball has tiles");

        let mut met = 0;
        for Wall { from, to, .. } in hive_edges(settings.size_mm, &bounds) {
            let (inner, outer) = (from.truncate(), to.truncate());
            let (inside, outside) = match (inner.length(), outer.length()) {
                (a, b) if a < radius - 1.0 && b > radius => (inner, outer),
                (a, b) if b < radius - 1.0 && a > radius => (outer, inner),
                _ => continue,
            };
            // Where the wall's own line crosses a circle a twentieth of a millimetre inside.
            let near = radius - 0.05;
            let (mut low, mut high) = (0.0, 1.0);
            for _ in 0..40 {
                let middle = Scalar::midpoint(low, high);
                if inside.lerp(outside, middle).length() < near {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            let at = inside.lerp(outside, low);
            assert!(
                covered(&boxes, Vec3::new(at.x, at.y, 0.0)),
                "a wall stops short of the side at {at}"
            );
            met += 1;
        }
        assert!(met > 3, "walls run into the side of a ball, got {met}");
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

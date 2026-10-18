use std::collections::HashMap;
use std::ops::RangeInclusive;

use core_csg::{CsgError, Solid};
use core_geometry::{
    Aabb, Bvh, DEFAULT_WELD_TOLERANCE, FastSet, Mesh, Scalar, Vec3, glam::IVec3, oriented_box,
    signed_volume, weld,
};
use rayon::prelude::*;

use crate::build::{DEFAULT_BUDGET_BYTES, FieldSettings, build};
use crate::cancel::Cancel;
use crate::error::VolumeError;
use crate::extract::extract;
use crate::grid::{VoxelGrid, index_in_tile, tile_of};
use crate::hollow::{COARSENINGS, MIN_WALL_MM};
use crate::sdf::{Quantised, Sdf, TILE_VALUES, encode};
use crate::sign::SignMode;

use super::pieces::{nest, shells};

/// Lattice points over the model's box: two dense arrays of them stay under a hundred
/// megabytes, and a 120 mm model still gets voxels under half a millimetre.
const LATTICE_POINTS: Scalar = 16_777_216.0;

/// Voxels the field carries past the surface; the wave reads only its sign.
const BAND_VOXELS: Scalar = 1.5;

/// How many times narrower than the widest front on either side a front must be to be a
/// neck. Under it a dip is the wave turning a corner, not a join.
const NECK_RATIO: Scalar = 2.0;

/// Levels either side a front's area is averaged over, so that a lattice row more or less
/// in one level is no dip.
const SMOOTHING: usize = 2;

/// Levels either side of the neck whose centres say which way the wave crossed it.
const SPAN: usize = 3;

/// Voxels the cut reaches past the furthest point of the neck's front.
const REACH_VOXELS: Scalar = 3.0;

/// Times the lattice is narrowed onto a piece the wave could not leave.
const ZOOMS: usize = 1;

/// Voxels a piece no material joins to the rest is grown out into the gap round it, so the
/// cut round it passes clear of its own surface.
const GAP_VOXELS: usize = 2;

/// Below this share of the model's volume, a wave that dies out without a neck was held
/// in by a join finer than its lattice, not by the end of the model.
const ISLAND_SHARE: Scalar = 0.5;

/// Times the cut is widened by half again when it does not close the neck off.
const WIDENINGS: usize = 2;

const UNREACHED: u32 = u32::MAX;
const OUTSIDE: u32 = u32::MAX - 1;

/// A model cut in two where the part a point is on joins the rest.
#[derive(Debug, Clone, Default)]
pub struct PartedOff {
    /// The side the point is on.
    pub part: Mesh,
    pub rest: Mesh,
}

/// Cuts `mesh` flat across the narrowest join between the part `at` is on and the rest
/// of it, in the coordinates `mesh` is in; see `docs/design/parts.md`.
///
/// [`VolumeError::NotOnModel`] where `at` is not on it, [`VolumeError::NoNeck`] where the
/// wave from it finds no join, [`VolumeError::NotSolid`] where the exact cut cannot read
/// it, and [`VolumeError::Cancelled`] when `cancel` gives up.
pub fn part_off(mesh: &Mesh, at: Vec3, cancel: Cancel<'_>) -> Result<PartedOff, VolumeError> {
    let welded = weld(mesh, DEFAULT_WELD_TOLERANCE).mesh;
    let bvh = Bvh::build(&welded);
    let volume_mm3 = signed_volume(&welded).abs();
    let bounds = welded.aabb().ok_or(VolumeError::EmptyMesh)?;
    let mut clip = None;
    let mut zooms = 0;
    let (wave, level) = loop {
        let field = inside_field(&welded, &bvh, clip.unwrap_or(bounds), cancel)?;
        let lattice = Lattice::over(&clip.unwrap_or(bounds), field.grid());
        let seed = seed(&field, &lattice, at).ok_or(VolumeError::NotOnModel)?;
        let wave = Wave::spread(&field, lattice, seed, cancel)?;
        let counts: Vec<usize> = wave.fronts.iter().map(Vec::len).collect();
        if let Some(level) = neck(&counts) {
            break (wave, level);
        }
        match wave.island(volume_mm3) {
            Some(around) if zooms < ZOOMS => {
                clip = Some(around);
                zooms += 1;
            }
            Some(_) => return cut_out(&solid_of(&welded)?, &wave, &field, cancel),
            None => return Err(VolumeError::NoNeck),
        }
    };
    let section = wave.section(level).ok_or(VolumeError::NoNeck)?;
    let body = solid_of(&welded)?;
    let mut half_mm = section.radius_mm;
    for _ in 0..=WIDENINGS {
        if let Some(reached) = wave.closed_off(&section, half_mm) {
            return cut(&body, &wave, &section, half_mm, &reached);
        }
        half_mm *= 1.5;
    }
    Err(VolumeError::NoNeck)
}

fn not_solid(error: &CsgError) -> VolumeError {
    VolumeError::NotSolid(error.to_string())
}

fn solid_of(mesh: &Mesh) -> Result<Solid, VolumeError> {
    Solid::new(mesh).map_err(|error| not_solid(&error))
}

/// `body` parted along the gap round the piece the wave filled, where no material the
/// finest lattice can see joins it to the rest: whatever does is thinner than printing
/// holds together.
fn cut_out(
    body: &Solid,
    wave: &Wave,
    field: &Sdf,
    cancel: Cancel<'_>,
) -> Result<PartedOff, VolumeError> {
    let cutter = occupied_surface(wave.lattice.grid, &wave.grown_into_gap(field), cancel)?;
    let cutter = solid_of(&cutter)?;
    let part = body
        .intersection(&cutter)
        .map_err(|error| not_solid(&error))?;
    let rest = body
        .difference(&cutter)
        .map_err(|error| not_solid(&error))?;
    let (part, rest) = (part.mesh(), rest.mesh());
    if part.faces.is_empty() || rest.faces.is_empty() {
        return Err(VolumeError::NoNeck);
    }
    Ok(PartedOff { part, rest })
}

/// The closed surface round `voxels`, half a voxel past their outermost centres.
fn occupied_surface(
    grid: VoxelGrid,
    voxels: &FastSet<IVec3>,
    cancel: Cancel<'_>,
) -> Result<Mesh, VolumeError> {
    let band_mm = grid.voxel_mm;
    let mut tiles: HashMap<IVec3, Box<[Quantised]>> = HashMap::new();
    for voxel in voxels {
        for near in around(*voxel, -1..=1) {
            tiles
                .entry(tile_of(near))
                .or_insert_with(|| vec![encode(band_mm, band_mm); TILE_VALUES].into());
        }
    }
    for voxel in voxels {
        if let Some(values) = tiles.get_mut(&tile_of(*voxel)) {
            values[index_in_tile(*voxel)] = encode(-band_mm, band_mm);
        }
    }
    let surface = extract(&Sdf::new(grid, band_mm, tiles, HashMap::new()), cancel);
    if cancel.asked() {
        return Err(VolumeError::Cancelled);
    }
    Ok(surface)
}

/// The field of `mesh` on a lattice fine enough for `LATTICE_POINTS` over `bounds`,
/// coarsened until it fits its budget. The field is the whole model's, being sparse; only
/// the wave's lattice is held to `bounds`.
fn inside_field(
    mesh: &Mesh,
    bvh: &Bvh,
    bounds: Aabb,
    cancel: Cancel<'_>,
) -> Result<Sdf, VolumeError> {
    let size = bounds.maxs - bounds.mins;
    let fits = (size.x * size.y * size.z / LATTICE_POINTS).cbrt();
    // The finest lattice a wall is ever cut on, three voxels to the thinnest wall.
    let mut voxel_mm = fits.max(MIN_WALL_MM / 3.0);
    let mut tries = 0;
    loop {
        let settings = FieldSettings {
            voxel_mm,
            band_voxels: BAND_VOXELS,
            iso_mm: 0.0,
            sign: SignMode::Auto,
            clip: None,
            budget_bytes: DEFAULT_BUDGET_BYTES,
        };
        match build(mesh, bvh, &settings, cancel) {
            Err(VolumeError::TooFine { fits_at_mm, .. }) if tries < COARSENINGS => {
                voxel_mm = fits_at_mm;
                tries += 1;
            }
            outcome => return outcome,
        }
    }
}

/// The box of lattice points the wave may spread over, held densely.
struct Lattice {
    grid: VoxelGrid,
    low: IVec3,
    size: IVec3,
}

impl Lattice {
    fn over(bounds: &Aabb, grid: VoxelGrid) -> Self {
        let low = grid.voxel(bounds.mins) - IVec3::splat(2);
        let size = grid.voxel(bounds.maxs) + IVec3::splat(3) - low;
        Self { grid, low, size }
    }

    fn len(&self) -> usize {
        (self.size.x as usize) * (self.size.y as usize) * (self.size.z as usize)
    }

    fn index(&self, voxel: IVec3) -> Option<usize> {
        let local = voxel - self.low;
        if local.cmplt(IVec3::ZERO).any() || local.cmpge(self.size).any() {
            return None;
        }
        Some(((local.z * self.size.y + local.y) * self.size.x + local.x) as usize)
    }

    fn voxel(&self, index: usize) -> IVec3 {
        let (width, depth) = (self.size.x as usize, self.size.y as usize);
        let local = [index % width, index / width % depth, index / width / depth]
            .map(|axis| i32::try_from(axis).unwrap_or(i32::MAX));
        self.low + IVec3::from_array(local)
    }

    fn position(&self, index: usize) -> Vec3 {
        self.grid.position(self.voxel(index))
    }
}

/// The lattice point inside the model nearest `at`, among those round the cell it is in.
fn seed(field: &Sdf, lattice: &Lattice, at: Vec3) -> Option<usize> {
    let cell = lattice.grid.voxel(at);
    around(cell, -1..=2)
        .filter(|voxel| field.value(*voxel) < 0.0)
        .filter_map(|voxel| lattice.index(voxel))
        .min_by(|a, b| {
            let away = |index: usize| lattice.position(index).distance_squared(at);
            away(*a).total_cmp(&away(*b))
        })
}

/// Lattice points `steps` away from `cell` on every axis.
fn around(cell: IVec3, steps: RangeInclusive<i32>) -> impl Iterator<Item = IVec3> {
    steps.clone().flat_map(move |z| {
        let steps = steps.clone();
        steps
            .clone()
            .flat_map(move |y| steps.clone().map(move |x| cell + IVec3::new(x, y, z)))
    })
}

const NEIGHBOURS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];

/// A breadth-first wave through the model's inside from one lattice point.
struct Wave {
    lattice: Lattice,
    seed: usize,
    /// Per lattice point, the level the wave reached it at, or `UNREACHED` or `OUTSIDE`.
    level: Vec<u32>,
    /// The lattice points of each level, its front.
    fronts: Vec<Vec<u32>>,
}

/// Where the wave crossed its neck: the plane through it and how far the front spread
/// across it.
struct Section {
    centre: Vec3,
    /// Unit normal, pointing into the part the wave started in.
    normal: Vec3,
    radius_mm: Scalar,
    /// Lattice points a few levels past the neck that the wave reached through it: the
    /// part closed off at the section must not reach them.
    beyond: Vec<u32>,
}

impl Wave {
    fn spread(
        field: &Sdf,
        lattice: Lattice,
        seed: usize,
        cancel: Cancel<'_>,
    ) -> Result<Self, VolumeError> {
        let mut level = vec![UNREACHED; lattice.len()];
        level[seed] = 0;
        let mut fronts = vec![vec![seed as u32]];
        while let Some(front) = fronts.last() {
            if cancel.asked() {
                return Err(VolumeError::Cancelled);
            }
            let next_level = fronts.len() as u32;
            let mut next = Vec::new();
            for index in front {
                let voxel = lattice.voxel(*index as usize);
                for step in NEIGHBOURS {
                    let Some(near) = lattice.index(voxel + step) else {
                        continue;
                    };
                    if level[near] != UNREACHED {
                        continue;
                    }
                    let inside = field.value(voxel + step) < 0.0;
                    level[near] = if inside { next_level } else { OUTSIDE };
                    if inside {
                        next.push(near as u32);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            fronts.push(next);
        }
        Ok(Self {
            lattice,
            seed,
            level,
            fronts,
        })
    }

    fn centre(&self, points: &[u32]) -> Option<Vec3> {
        let sum = points.iter().fold(Vec3::ZERO, |sum, index| {
            sum + self.lattice.position(*index as usize)
        });
        (!points.is_empty()).then(|| sum / points.len() as Scalar)
    }

    /// The box to look again in, round all the wave reached and as far again beyond it,
    /// where that is under `ISLAND_SHARE` of `volume_mm3`.
    fn island(&self, volume_mm3: Scalar) -> Option<Aabb> {
        let reached: usize = self.fronts.iter().map(Vec::len).sum();
        if reached as Scalar * self.lattice.grid.voxel_mm.powi(3) >= ISLAND_SHARE * volume_mm3 {
            return None;
        }
        let (low, high) = self.fronts.iter().flatten().fold(
            (Vec3::splat(Scalar::MAX), Vec3::splat(Scalar::MIN)),
            |(low, high), index| {
                let at = self.lattice.position(*index as usize);
                (low.min(at), high.max(at))
            },
        );
        let margin =
            Vec3::splat((high - low).max_element() / 2.0 + 2.0 * self.lattice.grid.voxel_mm);
        Some(Aabb::new(low - margin, high + margin))
    }

    /// Every lattice point the wave reached, grown `GAP_VOXELS` steps out into the empty
    /// space round them, short of anything that touches the rest of the model's inside.
    fn grown_into_gap(&self, field: &Sdf) -> FastSet<IVec3> {
        let reached = |voxel: IVec3| {
            self.lattice
                .index(voxel)
                .is_some_and(|index| self.level[index] < OUTSIDE)
        };
        let is_rest = |voxel: IVec3| field.value(voxel) < 0.0 && !reached(voxel);
        let mut grown: FastSet<IVec3> = self
            .fronts
            .iter()
            .flatten()
            .map(|index| self.lattice.voxel(*index as usize))
            .collect();
        let mut front: Vec<IVec3> = grown.iter().copied().collect();
        for _ in 0..GAP_VOXELS {
            let mut next = Vec::new();
            for voxel in front {
                for step in NEIGHBOURS {
                    let near = voxel + step;
                    let free = field.value(near) >= 0.0
                        && !NEIGHBOURS.iter().any(|side| is_rest(near + *side));
                    if free && grown.insert(near) {
                        next.push(near);
                    }
                }
            }
            front = next;
        }
        grown
    }

    /// The plane across the piece of the front at `level` the wave went on through,
    /// turned the way it crossed.
    fn section(&self, level: usize) -> Option<Section> {
        let neck = self.neck_piece(level);
        let before = self.followed(&neck, level, false);
        let mut after = self.followed(&neck, level, true);
        let centre = self.centre(&neck)?;
        let normal =
            (self.centre(&before.concat())? - self.centre(&after.concat())?).try_normalize()?;
        let spread = neck
            .iter()
            .map(|index| {
                let off = self.lattice.position(*index as usize) - centre;
                (off - normal * off.dot(normal)).length()
            })
            .fold(0.0, Scalar::max);
        Some(Section {
            centre,
            normal,
            radius_mm: spread + REACH_VOXELS * self.lattice.grid.voxel_mm,
            beyond: after.pop()?,
        })
    }

    /// The front at `level` in pieces that touch, by face, edge or corner.
    fn pieces(&self, level: usize) -> Vec<Vec<u32>> {
        let mut taken: FastSet<u32> = FastSet::default();
        let mut pieces = Vec::new();
        for start in &self.fronts[level] {
            if !taken.insert(*start) {
                continue;
            }
            let mut piece = vec![*start];
            let mut next = 0;
            while let Some(index) = piece.get(next).copied() {
                next += 1;
                let voxel = self.lattice.voxel(index as usize);
                for near in around(voxel, -1..=1).filter_map(|near| self.lattice.index(near)) {
                    if self.level[near] == level as u32 && taken.insert(near as u32) {
                        piece.push(near as u32);
                    }
                }
            }
            pieces.push(piece);
        }
        pieces
    }

    /// The piece of the front at `level` most of the wave beyond it came through; the
    /// others are corners it is still filling behind.
    fn neck_piece(&self, level: usize) -> Vec<u32> {
        let mut pieces = self.pieces(level);
        if pieces.len() < 2 {
            return pieces.pop().unwrap_or_default();
        }
        let mut came_from = vec![u16::MAX; self.level.len()];
        for (piece, points) in pieces.iter().enumerate().take(usize::from(u16::MAX)) {
            for index in points {
                came_from[*index as usize] = piece as u16;
            }
        }
        let mut carried = vec![0usize; pieces.len()];
        for index in self.fronts[level + 1..].iter().flatten() {
            let voxel = self.lattice.voxel(*index as usize);
            let from = NEIGHBOURS
                .iter()
                .filter_map(|step| self.lattice.index(voxel + *step))
                .map(|near| came_from[near])
                .find(|piece| *piece != u16::MAX);
            if let Some(piece) = from {
                came_from[*index as usize] = piece;
                carried[usize::from(piece)] += 1;
            }
        }
        let most = (0..pieces.len())
            .max_by_key(|piece| carried[*piece])
            .unwrap_or(0);
        pieces.swap_remove(most)
    }

    /// The lattice points `SPAN` levels on from `from` at `level`, or back from it, that
    /// the wave carried it to or brought it from, level by level.
    fn followed(&self, from: &[u32], level: usize, onward: bool) -> Vec<Vec<u32>> {
        let mut levels: Vec<Vec<u32>> = Vec::with_capacity(SPAN);
        for step in 1..=SPAN {
            let wanted = if onward {
                level + step
            } else {
                level.saturating_sub(step)
            };
            let current = levels.last().map_or(from, Vec::as_slice);
            let mut seen: FastSet<u32> = FastSet::default();
            for index in current {
                let voxel = self.lattice.voxel(*index as usize);
                for near in NEIGHBOURS
                    .iter()
                    .filter_map(|step| self.lattice.index(voxel + *step))
                {
                    if self.level[near] == wanted as u32 {
                        seen.insert(near as u32);
                    }
                }
            }
            if seen.is_empty() || step > level && !onward {
                break;
            }
            levels.push(seen.into_iter().collect());
        }
        levels
    }

    /// Which lattice points the seed still reaches once a square `half_mm` either way of
    /// the section's centre is closed, or `None` where the wave gets round it to the far
    /// side of the neck.
    fn closed_off(&self, section: &Section, half_mm: Scalar) -> Option<Vec<bool>> {
        let mut reached = vec![false; self.level.len()];
        reached[self.seed] = true;
        let mut front = vec![self.seed];
        while !front.is_empty() {
            let mut next = Vec::new();
            for index in front {
                let voxel = self.lattice.voxel(index);
                for step in NEIGHBOURS {
                    let Some(near) = self.lattice.index(voxel + step) else {
                        continue;
                    };
                    if reached[near]
                        || self.level[near] >= OUTSIDE
                        || crosses(section, half_mm, &self.lattice, index, near)
                    {
                        continue;
                    }
                    reached[near] = true;
                    next.push(near);
                }
            }
            front = next;
        }
        // A front is curved, so some of it lags on the part's side of a plane through its
        // centre; only what is past the plane says the wave got round.
        let leaked = section.beyond.iter().any(|index| {
            let off = self.lattice.position(*index as usize) - section.centre;
            reached[*index as usize] && off.dot(section.normal) < 0.0
        });
        (!leaked).then_some(reached)
    }
}

/// Whether the step from `from` to `to` passes through the square `half_mm` either way of
/// the section's centre in its plane.
fn crosses(section: &Section, half_mm: Scalar, lattice: &Lattice, from: usize, to: usize) -> bool {
    let (a, b) = (lattice.position(from), lattice.position(to));
    let (side_a, side_b) = (
        (a - section.centre).dot(section.normal),
        (b - section.centre).dot(section.normal),
    );
    if (side_a < 0.0) == (side_b < 0.0) {
        return false;
    }
    let mid = (a + b) / 2.0 - section.centre;
    let (across, along) = in_plane(section.normal);
    mid.dot(across).abs() < half_mm && mid.dot(along).abs() < half_mm
}

/// Two unit vectors square to `normal` and each other, with `across × along = normal`.
fn in_plane(normal: Vec3) -> (Vec3, Vec3) {
    let across = normal.any_orthonormal_vector();
    (across, normal.cross(across))
}

/// The level whose front is the narrowest against the widest fronts before and after it,
/// where that is under `1 / NECK_RATIO` of both.
fn neck(counts: &[usize]) -> Option<usize> {
    let smooth: Vec<Scalar> = (0..counts.len())
        .map(|level| {
            let window =
                &counts[level.saturating_sub(SMOOTHING)..(level + SMOOTHING + 1).min(counts.len())];
            window.iter().sum::<usize>() as Scalar / window.len() as Scalar
        })
        .collect();
    let mut before: Vec<Scalar> = vec![0.0; smooth.len()];
    for level in 1..smooth.len() {
        before[level] = before[level - 1].max(smooth[level - 1]);
    }
    let mut after: Vec<Scalar> = vec![0.0; smooth.len()];
    for level in (0..smooth.len().saturating_sub(1)).rev() {
        after[level] = after[level + 1].max(smooth[level + 1]);
    }
    let mut best: Option<(usize, Scalar)> = None;
    for level in 1..smooth.len().saturating_sub(1) {
        let ratio = before[level].min(after[level]) / smooth[level];
        if ratio >= NECK_RATIO && best.is_none_or(|(_, deepest)| ratio > deepest) {
            best = Some((level, ratio));
        }
    }
    best.map(|(level, _)| level)
}

/// `body` cut by the flat face of a box `half_mm` either way of the section, its pieces
/// sorted to the part or the rest by what the closed-off wave reached, and each side
/// joined back where the box's other faces went through it.
fn cut(
    body: &Solid,
    wave: &Wave,
    section: &Section,
    half_mm: Scalar,
    reached: &[bool],
) -> Result<PartedOff, VolumeError> {
    let (across, along) = in_plane(section.normal);
    let depth = section.normal * (half_mm / 2.0);
    let cutter = oriented_box(
        section.centre + depth,
        [across * half_mm, along * half_mm, depth],
    );
    let cutter = Solid::new(&cutter).map_err(|error| not_solid(&error))?;
    let inside = body
        .intersection(&cutter)
        .map_err(|error| not_solid(&error))?;
    let outside = body
        .difference(&cutter)
        .map_err(|error| not_solid(&error))?;
    let (part_in, rest_in) = sorted(&inside.mesh(), true, wave, section, reached);
    let (part_out, rest_out) = sorted(&outside.mesh(), false, wave, section, reached);
    let part = joined(&part_in, &part_out)?;
    let rest = joined(&rest_in, &rest_out)?;
    if part.faces.is_empty() || rest.faces.is_empty() {
        return Err(VolumeError::NoNeck);
    }
    Ok(PartedOff { part, rest })
}

fn joined(left: &Mesh, right: &Mesh) -> Result<Mesh, VolumeError> {
    let left = Solid::new(left).map_err(|error| not_solid(&error))?;
    let right = Solid::new(right).map_err(|error| not_solid(&error))?;
    Ok(left
        .union(&right)
        .map_err(|error| not_solid(&error))?
        .mesh())
}

/// The bodies of `mesh` split into those of the part and those of the rest. A body with
/// nothing to go by is the part's where it came from inside the cutter.
fn sorted(
    mesh: &Mesh,
    in_cutter: bool,
    wave: &Wave,
    section: &Section,
    reached: &[bool],
) -> (Mesh, Mesh) {
    let (mut part, mut rest) = (Mesh::default(), Mesh::default());
    for body in nest(shells(mesh)) {
        let (on_part, on_rest) = votes(&body.mesh, wave, section, reached);
        let is_part = match on_part.cmp(&on_rest) {
            std::cmp::Ordering::Equal => in_cutter,
            order => order.is_gt(),
        };
        if is_part {
            part.append(&body.mesh);
        } else {
            rest.append(&body.mesh);
        }
    }
    (part, rest)
}

/// How many of `mesh`'s vertices off the cut lie by a lattice point the closed-off wave
/// reached, and how many by one it did not.
fn votes(mesh: &Mesh, wave: &Wave, section: &Section, reached: &[bool]) -> (usize, usize) {
    let voxel_mm = wave.lattice.grid.voxel_mm;
    mesh.vertices
        .par_iter()
        .filter(|vertex| (**vertex - section.centre).dot(section.normal).abs() > voxel_mm)
        .filter_map(|vertex| {
            around(wave.lattice.grid.voxel(*vertex), -1..=2)
                .filter_map(|voxel| wave.lattice.index(voxel))
                .filter(|index| wave.level[*index] < OUTSIDE)
                .min_by(|a, b| {
                    let away =
                        |index: usize| wave.lattice.position(index).distance_squared(*vertex);
                    away(*a).total_cmp(&away(*b))
                })
        })
        .map(|index| if reached[index] { (1, 0) } else { (0, 1) })
        .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_narrow_front_between_two_wide_ones_is_the_neck() {
        let counts = [1, 4, 9, 16, 25, 25, 16, 4, 4, 4, 9, 30, 60, 30, 5];
        let level = neck(&counts).expect("the front narrows to 4 between 25 and 60");
        assert!(
            (7..=9).contains(&level),
            "the neck is the run of fours, got {level}"
        );
    }

    #[test]
    fn a_front_that_grows_and_dies_away_has_no_neck() {
        let counts = [1, 4, 9, 16, 25, 36, 25, 16, 9, 4, 1];
        assert_eq!(
            neck(&counts),
            None,
            "a ball's wave narrows only at its far end"
        );
    }
}

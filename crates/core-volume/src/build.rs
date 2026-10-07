use std::collections::HashSet;

use core_geometry::{Aabb, Bvh, ClosestPoint, Mesh, Scalar, Vec3, glam::IVec3};
use rayon::prelude::*;

use crate::cancel::Cancel;
use crate::error::VolumeError;
use crate::grid::{VoxelGrid, tile_of, within};
use crate::scatter::{Faces, seeds};
use crate::sdf::{Quantised, Sdf, TILE_VALUES, gap_runs};
use crate::sign::{SignMode, Signer};
use crate::sweep;

/// What a stored tile costs a hollowing run, in bytes.
///
/// Measured rather than derived: on a 60 mm ball at 0.05 mm, extraction peaks at 430 MB
/// over 111 615 tiles, and two other walls on the same ball give the same ratio. The
/// tile's own kilobyte is a quarter of it; the rest is the cavity mesh, resident beside
/// the field and again beside the model it is appended to. See
/// `docs/decisions/0063-a-field-is-built-to-a-memory-budget.md`.
pub const TILE_COST_BYTES: usize = 3_700;

/// What a tile of the carrier costs while the field is being filled, in bytes.
///
/// Its own five hundred faces, and the two rings kept beside it as halo. On the same ball
/// the field's peak is this times the carrier's tiles to within a seventh, over walls from
/// half a millimetre to six (ADR 0085).
const CARRIER_TILE_BYTES: usize = 4_300;

/// How coarse the field is, how far past the surface it reaches, which surface that is,
/// how it decides which side of it a voxel is on, and how much memory it may take.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldSettings {
    /// Lattice spacing in millimetres.
    pub voxel_mm: Scalar,
    /// How many voxels past the surface the field still carries a distance. An operator
    /// may not move the surface further than this.
    pub band_voxels: Scalar,
    /// Which surface of the mesh the field is of: the mesh itself at zero, its inward
    /// offset at a negative distance, its outward offset at a positive one. The band
    /// follows that surface, so a wall far thicker than the band costs no more than the
    /// mesh's own field; see
    /// `docs/decisions/0058-the-band-is-built-around-an-isosurface.md`.
    pub iso_mm: Scalar,
    pub sign: SignMode,
    /// The part of space worth filling, or `None` for wherever the mesh reaches. A field
    /// needed at one end of a model is not paid for over all of it.
    pub clip: Option<Aabb>,
    /// What the field may cost in bytes, or zero for no ceiling. The build is refused
    /// before a byte is spent rather than left to run the machine out of memory.
    pub budget_bytes: usize,
}

/// What a field may cost before it is refused, in bytes.
///
/// A gigabyte and a half is what a hollowing run can take on a desktop while the window
/// still holds the model, its hierarchy, the plate and the preview stack beside it.
pub const DEFAULT_BUDGET_BYTES: usize = 1_536 << 20;

impl Default for FieldSettings {
    fn default() -> Self {
        Self {
            voxel_mm: 0.1,
            band_voxels: 3.0,
            iso_mm: 0.0,
            sign: SignMode::Auto,
            clip: None,
            budget_bytes: DEFAULT_BUDGET_BYTES,
        }
    }
}

/// Builds the narrow-band field of `mesh`, through the hierarchy already built over it.
///
/// Only the tiles the band crosses are filled; what is deep inside is kept as runs of
/// whole tiles. See `docs/design/volume.md`. `cancel` is asked before and between the
/// phases and once a tile, and a run given up on returns [`VolumeError::Cancelled`].
pub fn build(
    mesh: &Mesh,
    bvh: &Bvh,
    settings: &FieldSettings,
    cancel: Cancel<'_>,
) -> Result<Sdf, VolumeError> {
    if mesh.aabb().is_none() {
        return Err(VolumeError::EmptyMesh);
    }
    if !settings.voxel_mm.is_finite() || settings.voxel_mm <= 0.0 {
        return Err(VolumeError::BadVoxelSize(settings.voxel_mm));
    }
    // A band narrower than a voxel can let the surface pass between two lattice points
    // without either of them noticing, which would lose a tile.
    if settings.band_voxels < 1.0 {
        return Err(VolumeError::BadBand(settings.band_voxels));
    }
    if !settings.iso_mm.is_finite() {
        return Err(VolumeError::BadIsoLevel(settings.iso_mm));
    }

    let band_mm = settings.band_voxels * settings.voxel_mm;
    let iso_mm = settings.iso_mm;
    let grid = VoxelGrid::new(settings.voxel_mm);
    let signer = Signer::new(mesh, settings.sign);
    let keep = settings.clip.map(|clip| clipped_tiles(grid, clip));

    let (rings, candidates) = reach(
        mesh,
        bvh,
        &signer,
        grid,
        band_mm,
        iso_mm,
        candidate_tiles(mesh, grid, band_mm, settings.clip),
        keep,
    );
    affordable(candidates.len(), sweep::carrier_tiles(&rings), settings)?;
    if cancel.asked() {
        return Err(VolumeError::Cancelled);
    }

    let faces = Faces::of(mesh);
    let wanted: HashSet<IVec3> = candidates.into_iter().collect();

    let tiles = sweep::fill(
        mesh,
        &faces,
        bvh,
        &signer,
        grid,
        band_mm,
        iso_mm,
        rings,
        &wanted,
        &seeds(mesh, &faces, sweep::carrier_grid(grid), keep),
        cancel,
    );
    if cancel.asked() {
        return Err(VolumeError::Cancelled);
    }

    let solid = gap_runs(walls(tiles.keys().copied(), settings.clip, grid), |tile| {
        let point = grid.tile_center(tile);
        bvh.closest(mesh, point)
            .is_some_and(|found| signed(&signer, mesh, point, &found) < iso_mm)
    });

    Ok(Sdf::new(grid, band_mm, tiles, solid))
}

/// The stored tiles, with a tile just outside each end of a clipped column added to them.
///
/// What is wholly inside the solid is worked out from the gaps between stored tiles, so a
/// column the clip cut short would have its deepest stretch read as a column that simply
/// ended — which is to say as empty space. The tile beyond the cut gives that stretch the
/// gap it needs; it carries no values and is never stored.
fn walls(
    stored: impl Iterator<Item = IVec3>,
    clip: Option<Aabb>,
    grid: VoxelGrid,
) -> impl Iterator<Item = IVec3> {
    let mut tiles: Vec<IVec3> = stored.collect();
    if let Some(clip) = clip {
        let (low, high) = clipped_tiles(grid, clip);
        let columns: HashSet<(i32, i32)> = tiles.iter().map(|tile| (tile.x, tile.y)).collect();
        for (x, y) in columns {
            tiles.push(IVec3::new(x, y, low.z - 1));
            tiles.push(IVec3::new(x, y, high.z + 1));
        }
    }
    tiles.into_iter()
}

/// What the tiles a build would fill cost, and whether the budget covers them.
///
/// Both counts are known before a single distance is asked for, so a lattice the machine
/// cannot hold is refused rather than discovered halfway through; see
/// `docs/decisions/0085-a-build-is-priced-by-its-larger-phase.md`.
///
/// The larger of the two phases, not their sum: the carrier is gone by the time the cavity
/// is meshed, and the cavity does not exist while the carrier is being swept.
fn affordable(tiles: usize, carried: usize, settings: &FieldSettings) -> Result<(), VolumeError> {
    let filling = tiles.saturating_mul(TILE_VALUES * size_of::<Quantised>())
        + carried.saturating_mul(CARRIER_TILE_BYTES);
    let meshing = tiles.saturating_mul(TILE_COST_BYTES);
    let needed_bytes = filling.max(meshing);
    if settings.budget_bytes == 0 || needed_bytes <= settings.budget_bytes {
        return Ok(());
    }

    // The tile count follows the surface's area over the square of the lattice, so the
    // lattice that fits is the current one scaled by the root of how far over it is.
    let over = needed_bytes as f64 / settings.budget_bytes as f64;
    Err(VolumeError::TooFine {
        needed_bytes,
        budget_bytes: settings.budget_bytes,
        voxel_mm: settings.voxel_mm,
        fits_at_mm: settings.voxel_mm * over.sqrt() as Scalar,
    })
}

/// Every tile a face can reach across `reach_mm`, within `clip` where there is one. A tile
/// nothing comes that near holds nothing worth storing.
fn candidate_tiles(
    mesh: &Mesh,
    grid: VoxelGrid,
    reach_mm: Scalar,
    clip: Option<Aabb>,
) -> HashSet<IVec3> {
    let reach = Vec3::splat(reach_mm);
    let keep = clip.map(|clip| clipped_tiles(grid, clip));

    (0..mesh.faces.len())
        .into_par_iter()
        .fold(HashSet::new, |mut tiles, face| {
            let Some(triangle) = mesh.triangle(face) else {
                return tiles;
            };
            let low = triangle.a.min(triangle.b).min(triangle.c) - reach;
            let high = triangle.a.max(triangle.b).max(triangle.c) + reach;
            let first = tile_of(grid.voxel(low));
            let last = tile_of(grid.voxel(high));

            for z in first.z..=last.z {
                for y in first.y..=last.y {
                    for x in first.x..=last.x {
                        let tile = IVec3::new(x, y, z);
                        if keep.is_none_or(|(low, high)| within(tile, low, high)) {
                            tiles.insert(tile);
                        }
                    }
                }
            }
            tiles
        })
        .reduce(HashSet::new, |mut tiles, other| {
            tiles.extend(other);
            tiles
        })
}

/// The lowest and highest tile a clip region reaches, both inclusive.
fn clipped_tiles(grid: VoxelGrid, clip: Aabb) -> (IVec3, IVec3) {
    let low = tile_of(grid.voxel(Vec3::new(clip.mins.x, clip.mins.y, clip.mins.z)));
    let high = tile_of(grid.voxel(Vec3::new(clip.maxs.x, clip.maxs.y, clip.maxs.z)));
    (low - IVec3::ONE, high + IVec3::ONE)
}

/// The shell between the mesh and the isosurface, and the tiles of it the band crosses.
///
/// The isosurface stands `|iso|` away from the mesh, which is many tiles for a thick
/// wall. Taking every tile within that radius would be the cube of it — a 6 mm wall on a
/// 0.14 mm lattice is two thousand tiles offered for every one worth having — so the
/// walk follows the distance instead: a tile is stepped through only while it is still
/// nearer the mesh than the isosurface's own band, which is a shell and not a ball.
///
/// The shell comes back as the rings the walk found it in, because that is the order the
/// sweep has to take it in: every tile of a ring has a neighbour in the ring before it.
#[allow(clippy::too_many_arguments)]
fn reach(
    mesh: &Mesh,
    bvh: &Bvh,
    signer: &Signer,
    grid: VoxelGrid,
    band_mm: Scalar,
    iso_mm: Scalar,
    seeds: HashSet<IVec3>,
    keep: Option<(IVec3, IVec3)>,
) -> (Vec<Vec<IVec3>>, Vec<IVec3>) {
    let edge_mm = band_mm + grid.tile_radius();
    let worth_filling = |distance: Scalar| (distance - iso_mm).abs() <= edge_mm;
    // A tile this much further from the mesh than the isosurface cannot lead to one that
    // is nearer it, because a step moves the distance by at most a tile. Behind the mesh,
    // away from its own isosurface, the walk stops at the tiles the surface itself
    // straddles: past those there is nothing the sweep could ever carry anywhere.
    let far_mm = iso_mm.abs() + edge_mm;
    let behind_mm = grid.tile_radius();
    let (low_mm, high_mm) = match iso_mm.partial_cmp(&0.0) {
        Some(std::cmp::Ordering::Less) => (-far_mm, behind_mm),
        Some(std::cmp::Ordering::Greater) => (-behind_mm, far_mm),
        _ => (-far_mm, far_mm),
    };
    let worth_stepping = |distance: Scalar| distance >= low_mm && distance <= high_mm;

    let mut visited: HashSet<IVec3> = seeds.clone();
    let mut frontier: Vec<IVec3> = seeds.into_iter().collect();
    let mut rings: Vec<Vec<IVec3>> = Vec::new();
    let mut found: Vec<IVec3> = Vec::new();

    while !frontier.is_empty() {
        let measured: Vec<(IVec3, Scalar)> = frontier
            .par_iter()
            .filter_map(|tile| {
                let center = grid.tile_center(*tile);
                let found = bvh.closest(mesh, center)?;
                Some((*tile, signed(signer, mesh, center, &found)))
            })
            .collect();

        frontier = Vec::new();
        rings.push(measured.iter().map(|(tile, _)| *tile).collect());
        for (tile, distance) in measured {
            if worth_filling(distance) {
                found.push(tile);
            }
            if !worth_stepping(distance) {
                continue;
            }
            for step in neighbours(tile) {
                if keep.is_none_or(|(low, high)| within(step, low, high)) && visited.insert(step) {
                    frontier.push(step);
                }
            }
        }
    }
    (rings, found)
}

/// The twenty-six tiles touching one, so the walk crosses a corner as well as a face.
fn neighbours(tile: IVec3) -> impl Iterator<Item = IVec3> {
    (-1..=1).flat_map(move |z| {
        (-1..=1).flat_map(move |y| {
            (-1..=1)
                .filter(move |x| *x != 0 || y != 0 || z != 0)
                .map(move |x| tile + IVec3::new(x, y, z))
        })
    })
}

/// The distance to the mesh, made negative on the inside of it.
fn signed(signer: &Signer, mesh: &Mesh, point: Vec3, found: &ClosestPoint) -> Scalar {
    if signer.is_inside(mesh, point, found) {
        -found.distance
    } else {
        found.distance
    }
}

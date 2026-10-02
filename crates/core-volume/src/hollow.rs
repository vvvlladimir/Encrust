use std::collections::HashSet;

use core_geometry::{Aabb, Bvh, Mesh, Scalar, Vec3, glam::IVec3, signed_volume};

use serde::{Deserialize, Serialize};

use crate::build::{DEFAULT_BUDGET_BYTES, FieldSettings, build};
use crate::csg::assemble;
use crate::drain::Channel;
use crate::error::VolumeError;
use crate::extract::extract;
use crate::grid::tile_of;
use crate::infill::{InfillSettings, lattice};
use crate::sdf::Sdf;
use crate::sign::SignMode;

/// How far past its own surface a hollowing field reaches.
///
/// Two voxels, not the three a field takes by default: a cube the cavity passes through
/// has no corner further than the root of three from it, so two is everything marching
/// cubes can read, and the build is a third less work for it.
pub(crate) const BAND_VOXELS: Scalar = 2.0;

/// The lattice a `precision` of 0 and of 1 ask for, in millimetres.
const COARSE_LATTICE_MM: Scalar = 0.8;
const FINE_LATTICE_MM: Scalar = 0.05;

/// Lattice squares the model's surface may be cut into, whatever precision asks for.
///
/// The cost of a field follows the surface's area over the square of the lattice, so that
/// area is what the ceiling is stated in. The model's longest side is not: a spire and a
/// block of the same height ask for the same spacing under it and cost nothing alike.
/// Seven million is a tenth of a millimetre on a ball the width of a Mars 4 plate, which
/// is where the side-based ceiling stood; see
/// `docs/decisions/0083-precision-is-capped-by-the-surface-it-pays-for.md`.
const MAX_LATTICE_SQUARES: Scalar = 7.0e6;

/// The thinnest wall the finest lattice can carry three voxels across, in millimetres.
///
/// Under this a wall is thinner than the lattice can resolve, so what comes out is not the
/// wall that was asked for. It is the only floor there is: what used to keep walls above
/// 0.3 mm was the field such a wall asked for, and that is no longer what it costs.
pub const MIN_WALL_MM: Scalar = 3.0 * FINE_LATTICE_MM;

/// The lattice a wall of `thickness_mm` on a model of `area_mm2` is cut on at
/// `precision`.
///
/// Precision runs from 0 to 1 and is the only handle on how smooth the cavity comes out,
/// the way every resin slicer states it. It moves the spacing geometrically, because the
/// cost is its square; a wall is always given three voxels whatever the setting, because a
/// wall the lattice cannot resolve is not a wall, and nothing is ever finer than the
/// model's own size allows.
pub fn lattice_mm(thickness_mm: Scalar, precision: Scalar, area_mm2: Scalar) -> Scalar {
    let precision = precision.clamp(0.0, 1.0);
    let asked = COARSE_LATTICE_MM * (FINE_LATTICE_MM / COARSE_LATTICE_MM).powf(precision);
    let resolves_the_wall = if thickness_mm.is_finite() && thickness_mm > 0.0 {
        thickness_mm / 3.0
    } else {
        FINE_LATTICE_MM
    };
    let the_model_allows = if area_mm2.is_finite() && area_mm2 > 0.0 {
        (area_mm2 / MAX_LATTICE_SQUARES).sqrt()
    } else {
        FINE_LATTICE_MM
    };
    asked
        .min(resolves_the_wall)
        .max(the_model_allows)
        .clamp(FINE_LATTICE_MM, COARSE_LATTICE_MM)
}

/// Which surface the wall is measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HollowMode {
    /// The wall grows inward from the model's own surface, which keeps its outside
    /// exactly as it was.
    #[default]
    Internal,
    /// The wall grows outward, so the model becomes the cavity of a mould around it.
    External,
    /// Internal, with the floor of the cavity taken out so the resin can run onto the
    /// plate. For a model standing on the plate; the opening is cut at its own underside.
    BottomThrough,
}

impl HollowMode {
    /// The modes a picker offers, in the order it draws them.
    pub const ALL: [Self; 3] = [Self::Internal, Self::External, Self::BottomThrough];

    pub fn label(self) -> &'static str {
        match self {
            Self::Internal => "Internal",
            Self::External => "External",
            Self::BottomThrough => "Bottom through",
        }
    }
}

/// A ball of `radius_mm` swept from `from` to `to`, inside which the wall stays solid, in
/// the model's own space.
///
/// It is what keeps a cavity out of a thin spar, away from a detail that would print
/// hollow and snap, or off a channel that is to come out as a pipe; the cavity has it
/// taken out of it before anything is meshed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Blocker {
    pub from: Vec3,
    pub to: Vec3,
    pub radius_mm: Scalar,
}

impl Blocker {
    pub fn ball(center: Vec3, radius_mm: Scalar) -> Self {
        Self {
            from: center,
            to: center,
            radius_mm,
        }
    }

    pub fn along(from: Vec3, to: Vec3, radius_mm: Scalar) -> Self {
        Self {
            from,
            to,
            radius_mm,
        }
    }

    /// How far inside this blocker `point` lies, in millimetres; negative outside it.
    pub fn depth_at(&self, point: Vec3) -> Scalar {
        let span = self.to - self.from;
        let squared = span.length_squared();
        let along = if squared > 0.0 {
            ((point - self.from).dot(span) / squared).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.radius_mm - (point - (self.from + span * along)).length()
    }
}

/// A sleeve of solid `wall_mm` thick around every leg of every channel, as blockers.
///
/// A channel is a pipe through the part, not a slot in it: without this the cavity opens
/// into the tube wherever the two cross, and what was dug as a duct comes out as a gap in
/// the infill. See `docs/design/hollowing.md`.
pub fn sleeves(channels: &[Channel], wall_mm: Scalar) -> Vec<Blocker> {
    channels
        .iter()
        .flat_map(|channel| {
            let radius = channel.diameter_mm / 2.0 + wall_mm;
            channel
                .points
                .windows(2)
                .map(move |pair| Blocker::along(pair[0], pair[1], radius))
        })
        .collect()
}

/// Everything one hollowing run needs.
#[derive(Debug, Clone, PartialEq)]
pub struct HollowSettings {
    /// Wall thickness in millimetres.
    pub thickness_mm: Scalar,
    pub mode: HollowMode,
    /// How smooth the cavity comes out, from 0 to 1. It is not the layer height: the
    /// outside of the model never passes through the lattice, so this only has to resolve
    /// the wall.
    pub precision: Scalar,
    pub sign: SignMode,
    /// What fills the cavity, or `None` to leave it empty.
    pub infill: Option<InfillSettings>,
    /// Where the wall stays solid.
    pub blockers: Vec<Blocker>,
    /// What the run may take in bytes, or zero for no ceiling. A lattice that would cost
    /// more is coarsened until it fits rather than left to run the machine out of memory.
    pub budget_bytes: usize,
}

impl HollowSettings {
    /// What a wall of `thickness_mm` asks for, at the default precision.
    pub fn for_wall(thickness_mm: Scalar) -> Self {
        Self {
            thickness_mm,
            ..Self::default()
        }
    }

    /// The lattice spacing this wall and precision come to on a model of `area_mm2`, in
    /// millimetres.
    pub fn voxel_mm(&self, area_mm2: Scalar) -> Scalar {
        lattice_mm(self.thickness_mm, self.precision, area_mm2)
    }
}

impl Default for HollowSettings {
    fn default() -> Self {
        Self {
            thickness_mm: 2.0,
            mode: HollowMode::default(),
            precision: 0.5,
            sign: SignMode::default(),
            infill: None,
            blockers: Vec::new(),
            budget_bytes: DEFAULT_BUDGET_BYTES,
        }
    }
}

/// A hollowed model, and how much resin the cavity saves.
#[derive(Debug, Clone, PartialEq)]
pub struct Hollowed {
    /// The whole solid, ready to slice: the outer surface, the cavity wound the other way
    /// and whatever fills it. The three are appended rather than welded, because the
    /// positive winding rule the rasteriser fills by already subtracts the cavity; see
    /// `docs/decisions/0059-a-hollow-is-the-model-with-its-cavity-appended.md`.
    pub mesh: Mesh,
    /// Volume the cavity takes out of the model, in cubic millimetres.
    pub cavity_mm3: Scalar,
    /// The lattice the cavity was actually cut on, in millimetres.
    pub voxel_mm: Scalar,
    /// Whether the lattice had to be coarsened from what precision asked for to stay
    /// inside the budget.
    pub coarsened: bool,
}

/// Turns a solid model into a shell of `thickness_mm`, with an optional lattice inside it.
///
/// The model itself is never remeshed: only the cavity comes out of a field, so the
/// outside keeps every triangle it was imported with. See `docs/design/hollowing.md`.
pub fn hollow(mesh: &Mesh, bvh: &Bvh, settings: &HollowSettings) -> Result<Hollowed, VolumeError> {
    shelled_to_budget(mesh, bvh, settings)
}

/// The shell alone, on the finest lattice that fits the budget.
fn shelled_to_budget(
    mesh: &Mesh,
    bvh: &Bvh,
    settings: &HollowSettings,
) -> Result<Hollowed, VolumeError> {
    let bounds = mesh.aabb().ok_or(VolumeError::EmptyMesh)?;
    if !settings.thickness_mm.is_finite() || settings.thickness_mm <= 0.0 {
        return Err(VolumeError::BadThickness(settings.thickness_mm));
    }

    let asked_mm = settings.voxel_mm(mesh.surface_area());
    let mut voxel_mm = asked_mm;
    for _ in 0..COARSENINGS {
        match cut(mesh, bvh, settings, bounds, voxel_mm) {
            // The lattice the field priced is the one that fits, taken a twentieth
            // coarser still: the second run has the blockers and the open floor to pay
            // for as well, and a run refused twice over is worse than one voxel blunter.
            Err(VolumeError::TooFine { fits_at_mm, .. }) => voxel_mm = fits_at_mm * 1.05,
            other => {
                return other.map(|mut hollowed| {
                    hollowed.coarsened = voxel_mm > asked_mm;
                    hollowed
                });
            }
        }
    }
    cut(mesh, bvh, settings, bounds, voxel_mm)
}

/// How many times a lattice may be coarsened before the run is given up on.
///
/// Each attempt prices the whole field before filling any of it, so a coarsening costs the
/// walk over the faces and nothing else; three is far more than the one step the estimate
/// normally needs.
pub(crate) const COARSENINGS: usize = 3;

/// One hollowing attempt, on a lattice of `voxel_mm`.
fn cut(
    mesh: &Mesh,
    bvh: &Bvh,
    settings: &HollowSettings,
    bounds: Aabb,
    voxel_mm: Scalar,
) -> Result<Hollowed, VolumeError> {
    let field = FieldSettings {
        voxel_mm,
        band_voxels: BAND_VOXELS,
        // The wall is the distance from the mesh to the surface the cavity is cut on:
        // inward for a shell, outward for a mould.
        iso_mm: match settings.mode {
            HollowMode::External => settings.thickness_mm,
            _ => -settings.thickness_mm,
        },
        sign: settings.sign,
        clip: None,
        budget_bytes: settings.budget_bytes,
    };
    let offset = build(mesh, bvh, &field)?;

    match settings.mode {
        HollowMode::External => Ok(mould(mesh, &offset, voxel_mm)),
        _ => shelled(mesh, bvh, offset, &field, bounds.mins.z, settings),
    }
}

/// The model with the cavity that `offset` bounds appended to it, and whatever stands in
/// that cavity appended after.
fn shelled(
    mesh: &Mesh,
    bvh: &Bvh,
    offset: Sdf,
    field: &FieldSettings,
    bottom_mm: Scalar,
    settings: &HollowSettings,
) -> Result<Hollowed, VolumeError> {
    let cavity = match settings.mode {
        HollowMode::BottomThrough => {
            let floor_mm = bottom_mm + settings.thickness_mm + field.voxel_mm;
            open_bottom(mesh, bvh, &offset, field, floor_mm, bottom_mm)?
        }
        _ => offset,
    };
    let cavity = blocked(cavity, &settings.blockers);

    let cavity_mesh = extract(&cavity);
    let hollow_mm3 = volume_of(&cavity_mesh);
    // Half the wall, so a strut is fused into the shell rather than left touching it, and
    // never so far that it could reach out through the wall.
    let bond_mm = settings.thickness_mm / 2.0;
    let filling = match &settings.infill {
        Some(infill) if !cavity.is_empty() => {
            lattice(&cavity, infill, settings.precision, bond_mm)?
        }
        _ => Mesh::default(),
    };

    // Grown to what the three parts come to before the first is copied: a mesh that
    // doubles its way there holds the old copy and the new one at once, and the cavity of
    // a real model is millions of triangles.
    let mut whole = Mesh::new(
        Vec::with_capacity(
            mesh.vertices.len() + cavity_mesh.vertices.len() + filling.vertices.len(),
        ),
        Vec::with_capacity(mesh.faces.len() + cavity_mesh.faces.len() + filling.faces.len()),
    );
    append(&mut whole, mesh);
    append(&mut whole, &flipped(&cavity_mesh));
    drop(cavity_mesh);
    append(&mut whole, &filling);

    // The lattice overlaps itself at every junction, so its own volume over-reads. What it
    // takes back out of the cavity is the density it was asked for.
    let filled = settings.infill.map_or(0.0, |infill| infill.density);
    Ok(Hollowed {
        mesh: whole,
        cavity_mm3: (hollow_mm3 * (1.0 - filled)).max(0.0),
        voxel_mm: field.voxel_mm,
        coarsened: false,
    })
}

/// The model turned inside out: the grown surface becomes the outside and the model's own
/// becomes the cavity.
fn mould(mesh: &Mesh, grown: &Sdf, voxel_mm: Scalar) -> Hollowed {
    let mut whole = extract(grown);
    append(&mut whole, &flipped(mesh));
    Hollowed {
        mesh: whole,
        cavity_mm3: volume_of(mesh),
        voxel_mm,
        coarsened: false,
    }
}

/// The cavity with its floor taken out: its cross-section just above the floor, carried
/// down through the bottom of the model.
///
/// The prism is clipped against the model's own field rather than against a plane, so the
/// cavity can never reach outside the solid — which is what would otherwise leave a rim of
/// material with nothing around it once the two are sliced together.
fn open_bottom(
    mesh: &Mesh,
    bvh: &Bvh,
    cavity: &Sdf,
    field: &FieldSettings,
    floor_mm: Scalar,
    bottom_mm: Scalar,
) -> Result<Sdf, VolumeError> {
    // Only the floor is cut against the model, so the model's own field is built for the
    // slab the floor stands in and not over the whole part, which on a tall model is the
    // difference between a second field and a fortieth of one.
    let mut slab = mesh.aabb().ok_or(VolumeError::EmptyMesh)?;
    slab.maxs.z = floor_mm + field.band_voxels * field.voxel_mm;
    let solid = build(
        mesh,
        bvh,
        &FieldSettings {
            iso_mm: 0.0,
            clip: Some(slab),
            ..*field
        },
    )?;

    let grid = cavity.grid();
    let floor_voxel = grid.voxel(Vec3::new(0.0, 0.0, floor_mm)).z;
    let floor_tile = tile_of(IVec3::new(0, 0, floor_voxel)).z;
    let bottom_tile = tile_of(grid.voxel(Vec3::new(0.0, 0.0, bottom_mm))).z;

    let mut candidates: HashSet<IVec3> = cavity.tile_keys().collect();
    candidates.extend(solid.tile_keys());
    let columns: HashSet<(i32, i32)> = cavity.tile_keys().map(|tile| (tile.x, tile.y)).collect();
    for (x, y) in columns {
        for z in bottom_tile..=floor_tile {
            candidates.insert(IVec3::new(x, y, z));
        }
    }

    let band_mm = cavity.band_mm().min(solid.band_mm());
    Ok(assemble(grid, band_mm, candidates, |voxel| {
        // Above the floor the cavity is itself: the model's field there is inside the
        // solid and would never have won the comparison, and it is no longer built.
        if voxel.z >= floor_voxel {
            return cavity.value(voxel);
        }
        let lifted = IVec3::new(voxel.x, voxel.y, floor_voxel);
        cavity.value(lifted).max(solid.value(voxel))
    }))
}

/// The cavity with every blocker taken out of it.
fn blocked(cavity: Sdf, blockers: &[Blocker]) -> Sdf {
    if blockers.is_empty() {
        return cavity;
    }

    let grid = cavity.grid();
    let band_mm = cavity.band_mm();
    let mut candidates: HashSet<IVec3> = cavity.tile_keys().collect();
    for blocker in blockers {
        let reach = Vec3::splat(blocker.radius_mm + band_mm);
        let first = tile_of(grid.voxel(blocker.from.min(blocker.to) - reach));
        let last = tile_of(grid.voxel(blocker.from.max(blocker.to) + reach));
        for z in first.z..=last.z {
            for y in first.y..=last.y {
                for x in first.x..=last.x {
                    candidates.insert(IVec3::new(x, y, z));
                }
            }
        }
    }

    assemble(grid, band_mm, candidates, |voxel| {
        let point = grid.position(voxel);
        blockers.iter().fold(cavity.value(voxel), |value, blocker| {
            value.max(blocker.depth_at(point))
        })
    })
}

/// The same surface wound the other way, so that what it encloses is taken out of
/// whatever it is appended to.
fn flipped(mesh: &Mesh) -> Mesh {
    Mesh::new(
        mesh.vertices.clone(),
        mesh.faces.iter().map(|[a, b, c]| [*a, *c, *b]).collect(),
    )
}

/// What a closed surface encloses, in cubic millimetres, whichever way it is wound.
fn volume_of(mesh: &Mesh) -> Scalar {
    signed_volume(mesh).abs()
}

fn append(whole: &mut Mesh, part: &Mesh) {
    let offset = whole.vertices.len() as u32;
    whole.vertices.extend_from_slice(&part.vertices);
    whole.faces.extend(
        part.faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
}

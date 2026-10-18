use core_csg::{CsgError, Solid};
use core_geometry::{Bvh, Mesh, Scalar, signed_volume, split};

use crate::build::{DEFAULT_BUDGET_BYTES, FieldSettings, build};
use crate::cancel::Cancel;
use crate::error::VolumeError;
use crate::extract::extract;
use crate::hollow::{COARSENINGS, lattice_mm};
use crate::sign::SignMode;

/// Voxels across the half-width of the narrowest join that is looked for, so erosion
/// reads it to within a few tenths.
const VOXELS_PER_HALF_NECK: Scalar = 2.5;

/// Voxels the field carries past the surface it is of.
const BAND_VOXELS: Scalar = 2.0;

/// Voxels the cutter around a core reaches past the body the core came from, so the
/// cutter's own surface stays off the model's except across a join.
const CUTTER_SLACK_VOXELS: Scalar = 2.0;

/// The bodies `body` falls into where it is joined by something narrower than
/// `neck_mm`: each core left by eroding it half that width, grown back past where it
/// was, and cut out of it in turn. Cores holding less than `smallest_mm3` are not
/// counted; see `docs/design/parts.md`.
///
/// Answers `body` alone where it has no such join. A body the exact cut cannot read is a
/// `NotSolid` error, for the caller to keep whole.
pub(crate) fn parted(
    body: &Mesh,
    neck_mm: Scalar,
    smallest_mm3: Scalar,
    cancel: Cancel<'_>,
) -> Result<Vec<Mesh>, VolumeError> {
    let half = neck_mm / 2.0;
    let bvh = Bvh::build(body);
    let (eroded, voxel_mm) = field_surface(body, &bvh, -half, half, cancel)?;
    let mut cores: Vec<Mesh> = split(&eroded)
        .into_iter()
        .filter(|core| signed_volume(core) >= smallest_mm3)
        .collect();
    if cores.len() < 2 {
        return Ok(vec![body.clone()]);
    }
    cores.sort_by(|a, b| signed_volume(a).total_cmp(&signed_volume(b)));
    // The largest core keeps whatever the others' cutters leave.
    cores.pop();
    let grow = half + CUTTER_SLACK_VOXELS * voxel_mm;
    let mut rest = Solid::new(body).map_err(|error| not_solid(&error))?;
    let mut pieces = Vec::with_capacity(cores.len() + 1);
    for core in &cores {
        let core_bvh = Bvh::build(core);
        let (cutter, _) = field_surface(core, &core_bvh, grow, half, cancel)?;
        let cutter = Solid::new(&cutter).map_err(|error| not_solid(&error))?;
        pieces.push(
            rest.intersection(&cutter)
                .map_err(|error| not_solid(&error))?
                .mesh(),
        );
        rest = rest
            .difference(&cutter)
            .map_err(|error| not_solid(&error))?;
    }
    pieces.push(rest.mesh());
    pieces.retain(|piece| !piece.faces.is_empty());
    Ok(pieces)
}

fn not_solid(error: &CsgError) -> VolumeError {
    VolumeError::NotSolid(error.to_string())
}

/// The surface `iso_mm` off `mesh`, outward where positive, on a lattice fine enough for
/// `half_mm` and coarsened until the field fits its budget. Answers the lattice too.
fn field_surface(
    mesh: &Mesh,
    bvh: &Bvh,
    iso_mm: Scalar,
    half_mm: Scalar,
    cancel: Cancel<'_>,
) -> Result<(Mesh, Scalar), VolumeError> {
    let fine = half_mm / VOXELS_PER_HALF_NECK;
    let mut voxel_mm = fine.max(lattice_mm(Scalar::INFINITY, 1.0, mesh.surface_area()));
    let mut tries = 0;
    loop {
        let settings = FieldSettings {
            voxel_mm,
            band_voxels: BAND_VOXELS,
            iso_mm,
            sign: SignMode::Auto,
            clip: None,
            budget_bytes: DEFAULT_BUDGET_BYTES,
        };
        match build(mesh, bvh, &settings, cancel) {
            Ok(field) => {
                let surface = extract(&field, cancel);
                if cancel.asked() {
                    return Err(VolumeError::Cancelled);
                }
                return Ok((surface, voxel_mm));
            }
            Err(VolumeError::TooFine { fits_at_mm, .. }) if tries < COARSENINGS => {
                voxel_mm = fits_at_mm;
                tries += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

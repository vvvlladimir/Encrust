//! A picture tiled over a model and pressed into it (`docs/design/document.md`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use core_edit::{Edit, ObjectId};
use core_geometry::{Bvh, Heightmap, Mat3, Mesh, Region, Scalar, Vec3, transform_mesh};
use core_mesh_io::decode_picture;
use core_volume::{
    AngleMask, Cancel, Relief, ReliefBasis, ReliefSettings, Tiling, VolumeError,
    press_painted_tiled, press_tiled_reusing,
};

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;
use crate::hollow::hollowed_again;
use core_engine::project::hollow_of;

/// What a relief asks: the picture, where it is tiled and how it is pressed.
#[derive(Debug, Clone, PartialEq)]
pub struct ReliefAsked {
    pub image: PathBuf,
    pub tiling: Tiling,
    pub settings: ReliefSettings,
}

impl ReliefAsked {
    /// What a relief edit asks, the plate's up being the mesh's own +Z; `None` for any other
    /// edit.
    pub fn of(edit: &Edit) -> Option<Self> {
        let Edit::Relief {
            depth_mm,
            image,
            tile_mm,
            angle_deg,
            invert,
            symmetric,
            smoothing_px,
            hold_bottom_deg,
            hold_top_deg,
            precision,
            ..
        } = edit
        else {
            return None;
        };
        Some(Self {
            image: image.clone(),
            tiling: Tiling {
                tile_mm: *tile_mm,
                angle_deg: *angle_deg,
            },
            settings: ReliefSettings {
                amplitude_mm: *depth_mm,
                precision: *precision,
                invert: *invert,
                symmetric: *symmetric,
                smoothing_px: *smoothing_px,
                mask: AngleMask {
                    bottom_deg: *hold_bottom_deg,
                    top_deg: *hold_top_deg,
                    ..AngleMask::default()
                },
                ..ReliefSettings::default()
            },
        })
    }
}

/// A model with the picture pressed in.
#[derive(Debug, Clone)]
pub struct Relieved {
    pub mesh: Mesh,
    /// The lattice the relief came out on, mm.
    pub voxel_mm: Scalar,
    /// Whether the memory budget made the lattice coarser than precision asked for.
    pub coarsened: bool,
}

/// The picture at `path` as one height per pixel, read as a texture would be.
pub fn read_picture(path: &Path) -> Result<Heightmap, ApplyError> {
    let bad = |reason: String| ApplyError::BadPicture {
        path: path.to_path_buf(),
        reason,
    };
    let bytes = std::fs::read(path).map_err(|error| bad(error.to_string()))?;
    decode_picture(&path.display().to_string(), &bytes)
        .map(|picture| picture.heights)
        .map_err(|error| bad(error.to_string()))
}

/// `picture` tiled and pressed into `mesh`: over a field where nothing is `painted`, which
/// stands on `basis` where it fits and answers the one it stood on, or by moving the painted
/// faces alone, which keeps a basis for no other press.
pub fn press_picture(
    mesh: &Mesh,
    bvh: &Bvh,
    (image, tiling): (&Heightmap, Tiling),
    settings: &ReliefSettings,
    painted: Option<&Region>,
    basis: Option<Arc<ReliefBasis>>,
    cancel: Cancel<'_>,
) -> Result<(Relief, Option<Arc<ReliefBasis>>), VolumeError> {
    match painted {
        Some(painted) => press_painted_tiled(mesh, image, tiling, settings, painted, cancel)
            .map(|relief| (relief, None)),
        None => press_tiled_reusing(mesh, bvh, image, tiling, settings, basis, cancel)
            .map(|(relief, basis)| (relief, Some(basis))),
    }
}

/// Presses the picture `asked` names into `mesh`, at the size and in the space it is given:
/// over the whole of it, or the faces of `painted` alone. `object` names the model in an error.
pub fn relieved(
    mesh: &Mesh,
    asked: &ReliefAsked,
    painted: Option<&Region>,
    cancel: Cancel<'_>,
    object: ObjectId,
) -> Result<Relieved, ApplyError> {
    let image = read_picture(&asked.image)?;
    let picture = (&image, asked.tiling);
    let bvh = Bvh::build(mesh);
    let (relief, _) = press_picture(mesh, &bvh, picture, &asked.settings, painted, None, cancel)
        .map_err(|error| match error {
            VolumeError::Cancelled => ApplyError::Cancelled,
            other => ApplyError::Failed {
                object,
                reason: other.to_string(),
            },
        })?;
    Ok(Relieved {
        mesh: relief.mesh,
        voxel_mm: relief.voxel_mm,
        coarsened: relief.coarsened,
    })
}

/// Presses the picture into the printed size of the mesh of `id`, the scale and stretch
/// going into the mesh as the window's Relief tool does.
pub(crate) fn apply_relief(
    document: &mut Document,
    id: ObjectId,
    asked: &ReliefAsked,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let transform = state.transform;
    let hollow = hollow_of(&state.hollow, meshes);
    let sized = transform_mesh(&meshes.source, transform.sizing());
    let mut asked = asked.clone();
    asked.settings.mask.up = transform.rotation.inverse() * Vec3::Z;
    let painted = Some(&state.relief_painted).filter(|region| !region.is_empty());
    let done = relieved(&sized, &asked, painted, cancel, id)?;
    document.reshape(id, done.mesh);
    document.set_transform(
        id,
        core_geometry::Transform {
            scale: Vec3::ONE,
            stretch: Mat3::IDENTITY,
            ..transform
        },
    );
    hollowed_again(document, id, (&hollow, transform.sizing().linear()), cancel)?;
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report: Report::Relieved {
            voxel_mm: done.voxel_mm,
            coarsened: done.coarsened,
        },
    })
}

#[cfg(test)]
mod tests;

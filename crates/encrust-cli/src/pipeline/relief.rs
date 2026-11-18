use crate::args::ImportArgs;
use anyhow::{Context, Result};
use core_document::{ApplyError, ReliefAsked, Relieved, relieved};
use core_edit::ObjectId;
use core_geometry::{Bvh, Heightmap, Mesh, UvMap, Welded};
use core_mesh_io::Loaded;
use core_volume::{AngleMask, Cancel, ReliefSettings, Tiling, press};
use std::time::Instant;

/// Presses `--relief-image` or the file's own texture into the welded mesh, where
/// `--relief` asked for it.
pub(super) fn press_relief(
    mesh: Mesh,
    welded: &Welded,
    loaded: &Loaded,
    args: &ImportArgs,
    talk: bool,
) -> Result<Mesh> {
    let Some(amplitude_mm) = args.relief else {
        return Ok(mesh);
    };
    // Pressed after placement, so the plate's up is the mesh's own +Z.
    let settings = ReliefSettings {
        amplitude_mm,
        precision: args.precision,
        invert: args.relief_invert,
        symmetric: args.relief_symmetric,
        smoothing_px: args.relief_smooth,
        mask: AngleMask {
            bottom_deg: args.relief_hold_bottom,
            top_deg: args.relief_hold_top,
            ..AngleMask::default()
        },
        ..ReliefSettings::default()
    };
    let started = Instant::now();
    let (relief, source) = match &args.relief_image {
        Some(path) => {
            let asked = ReliefAsked {
                image: path.clone(),
                tiling: Tiling {
                    tile_mm: args.relief_tile,
                    angle_deg: args.relief_angle,
                },
                settings,
            };
            let relief =
                relieved(&mesh, &asked, None, Cancel::never(), ObjectId::UNSET).map_err(refused)?;
            let source = format!("{} tiled every {} mm", path.display(), asked.tiling.tile_mm);
            (relief, source)
        }
        None => {
            let (uvs, heights) = texture_of(welded, loaded)?;
            let relief = press(&mesh, &Bvh::build(&mesh), &uvs, &heights, &settings)
                .context("pressing the texture into the model")?;
            let relief = Relieved {
                mesh: relief.mesh,
                voxel_mm: relief.voxel_mm,
                coarsened: relief.coarsened,
            };
            (relief, format!("{} image(s)", heights.len()))
        }
    };

    if talk {
        let lattice = match relief.coarsened {
            true => format!(
                "{:.2} mm lattice, coarsened to fit the memory budget",
                relief.voxel_mm
            ),
            false => format!("{:.2} mm lattice", relief.voxel_mm),
        };
        println!(
            "Relief: {amplitude_mm} mm of {source} pressed in on a {lattice}, {} faces \
             became {}, took {:.1}s\n",
            mesh.faces.len(),
            relief.mesh.faces.len(),
            started.elapsed().as_secs_f32()
        );
    }
    Ok(relief.mesh)
}

/// The file's own map, taken through the weld first: welding is the one step that
/// renumbers the faces it is indexed by. And the images it addresses, as heights.
pub(super) fn texture_of(welded: &Welded, loaded: &Loaded) -> Result<(UvMap, Vec<Heightmap>)> {
    let uvs = loaded
        .uvs
        .as_ref()
        .context("--relief needs a model carrying texture coordinates, or --relief-image")?
        .without(&welded.dropped);
    if loaded.textures.is_empty() {
        anyhow::bail!("--relief needs a texture beside the model, or --relief-image");
    }
    let heights = loaded
        .textures
        .iter()
        .map(|texture| {
            texture
                .decode()
                .with_context(|| format!("reading {}", texture.name))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((uvs, heights))
}

/// Why a picture could not be pressed, without the object id a command line has none of.
fn refused(error: ApplyError) -> anyhow::Error {
    match error {
        ApplyError::Failed { reason, .. } => {
            anyhow::anyhow!("pressing the picture into the model: {reason}")
        }
        other => anyhow::anyhow!("{other}"),
    }
}

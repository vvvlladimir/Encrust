//! `plate.toml`: a plate of several models written by hand or by a script, each with its
//! own placement, supports and wall. The format is `docs/cli.md`; ADR 0179.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_geometry::{Scalar, Vec2, Vec3};
use serde::Deserialize;

use crate::args::{JobArgs, SupportPreset};
use crate::hollowing::Mode;
use crate::pipeline::Watch;
use crate::profiles::{self, Selection};
use crate::stage::{Shaping, Staged, plate_of, refuse_one_model_flags};

/// The whole file. A key it does not know is refused, so a typo is not a setting quietly
/// left at its default.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlateFile {
    pub printer: Option<String>,
    pub resin: Option<String>,
    pub layer_height_mm: Option<Scalar>,
    #[serde(default)]
    pub arrange: bool,
    #[serde(rename = "model")]
    pub models: Vec<ModelEntry>,
}

/// One model on the plate.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    /// The mesh, relative to the plate file.
    pub path: PathBuf,
    /// Degrees around X, Y and Z, in that order.
    pub rotate: Option<[Scalar; 3]>,
    pub scale: Option<Scale>,
    /// Where the middle of the footprint goes, plate millimetres.
    pub position: Option<[Scalar; 2]>,
    pub supports: Option<SupportPreset>,
    pub hollow: Option<Wall>,
}

/// One factor for every axis, or one each.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(untagged)]
pub enum Scale {
    Uniform(Scalar),
    PerAxis([Scalar; 3]),
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wall {
    pub wall_mm: Scalar,
    #[serde(default)]
    pub mode: Mode,
}

/// Reads a plate file.
pub fn read(path: &Path) -> Result<PlateFile> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let plate: PlateFile =
        toml::from_str(&text).with_context(|| format!("{} is not a plate file", path.display()))?;
    if plate.models.is_empty() {
        bail!("{} names no [[model]]", path.display());
    }
    Ok(plate)
}

/// Stages the plate `path` describes. A flag wins over the file, and the file over the
/// profiles.
pub fn stage(path: &Path, arrange: bool, job: &JobArgs, watch: &Watch) -> Result<Staged> {
    let plate = read(path)?;
    let arrange = arrange || plate.arrange;
    if arrange && plate.models.iter().any(|model| model.position.is_some()) {
        bail!("a plate that is arranged places every model itself; drop `position` or `arrange`");
    }

    let flags = &job.profile;
    let chosen = profiles::resolve(&Selection {
        printer_id: flags.printer.as_deref().or(plate.printer.as_deref()),
        printer_path: flags.profile.as_deref(),
        resin_id: flags.resin.as_deref().or(plate.resin.as_deref()),
        resin_path: flags.material.as_deref(),
    })?;
    let layer_height = job
        .slicing
        .layer_height
        .or(plate.layer_height_mm)
        .unwrap_or(chosen.material.layer_height_mm);

    let base = Shaping::of(job)?;
    if plate.models.len() > 1 {
        refuse_one_model_flags(&base)?;
    }
    let beside = path.parent().unwrap_or(Path::new("."));
    let entries: Vec<_> = plate
        .models
        .iter()
        .map(|model| (beside.join(&model.path), shaping_of(&base, model)))
        .collect();
    plate_of(&entries, arrange, layer_height, job, &chosen, watch)
}

/// What the flags ask of every model, with this entry's own settings where a flag was not
/// given.
fn shaping_of(flags: &Shaping, model: &ModelEntry) -> Shaping {
    let mut shaping = flags.clone();
    let transform = &mut shaping.import.transform;
    transform.rotate = transform.rotate.or(model.rotate.map(Vec3::from_array));
    transform.scale = transform.scale.or(model.scale.map(|scale| match scale {
        Scale::Uniform(factor) => Vec3::splat(factor),
        Scale::PerAxis(factors) => Vec3::from_array(factors),
    }));
    if shaping.supports.is_none() {
        shaping.supports = model.supports.map(SupportPreset::profile);
    }
    if !shaping.hollow.wanted()
        && let Some(wall) = model.hollow
    {
        shaping.hollow.thickness_mm = Some(wall.wall_mm);
        shaping.hollow.mode = wall.mode;
    }
    shaping.position = model.position.map(Vec2::from_array);
    shaping
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plate_file_reads_every_key_it_documents() {
        let plate: PlateFile = toml::from_str(
            r#"
            printer = "elegoo-mars-4-ultra"
            resin = "standard-grey"
            layer_height_mm = 0.03
            [[model]]
            path = "a.stl"
            rotate = [0, 0, 45]
            scale = 1.5
            position = [60, 40]
            supports = "medium"
            [[model]]
            path = "b.stl"
            scale = [1, 2, 1]
            hollow = { wall_mm = 2.0, mode = "internal" }
            [[model]]
            path = "c.stl"
            hollow = { wall_mm = 1.0, mode = "external" }
            "#,
        )
        .expect("the documented example parses");

        assert_eq!(plate.models.len(), 3);
        assert_eq!(plate.models[0].rotate, Some([0.0, 0.0, 45.0]));
        assert!(matches!(plate.models[0].scale, Some(Scale::Uniform(factor)) if factor == 1.5));
        assert!(matches!(
            plate.models[1].hollow,
            Some(Wall {
                mode: Mode::Internal,
                ..
            })
        ));
        assert!(matches!(
            plate.models[2].hollow,
            Some(Wall {
                mode: Mode::External,
                ..
            })
        ));
    }

    #[test]
    fn a_misspelt_key_is_refused_rather_than_ignored() {
        let read = toml::from_str::<PlateFile>("[[model]]\npath = \"a.stl\"\nrotation = [0, 0, 1]");
        assert!(read.is_err(), "`rotation` is not a key; `rotate` is");
    }
}

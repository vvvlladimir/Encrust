//! A sliced file open under the layer slider: what it states about itself, and the
//! plate it was opened over (ADR 0151, 0202).

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use anyhow::{Result, bail};
use core_format::{OpenFile, ReadSeek, SlicedFile};
use core_geometry::Scalar;
use core_raster::{Grey, PixelPitch, RasterSettings, Shading};

use core_pipeline::open;

use crate::files::Handed;

use super::*;

/// A file's own fingerprint for the texture cache: its name, and what it was last written —
/// or, for bytes handed over, which bytes they are.
///
/// A file opened twice over an edit must not show the first one's cached picture.
pub(super) fn file_fingerprint(file: &Handed) -> u64 {
    let mut hasher = DefaultHasher::new();
    let path = file.path();
    path.hash(&mut hasher);
    if let Handed::Bytes { bytes, .. } = file {
        bytes.as_ptr().hash(&mut hasher);
        bytes.len().hash(&mut hasher);
    } else if let Ok(meta) = std::fs::metadata(path) {
        meta.len().hash(&mut hasher);
        if let Ok(modified) = meta.modified() {
            modified.hash(&mut hasher);
        }
    }
    hasher.finish()
}

impl Preview {
    /// Opens a sliced file and shows it in place of the plate.
    ///
    /// Only its tables are read: a layer is decoded when the slider lands on it, so opening
    /// a stack of thousands costs the same as opening one of ten.
    pub fn read_file(&mut self, file: &Handed, plate: u64) -> Result<()> {
        let path = file.path();
        let source: Box<dyn ReadSeek> = file
            .reader()
            .with_context(|| format!("cannot read {}", path.display()))?;
        let open = open(path, source).with_context(|| format!("cannot open {}", path.display()))?;
        let facts = open.facts().clone();
        if facts.layer_count() == 0 {
            bail!("{} has no layers", path.display());
        }
        self.job = None;
        self.full = None;
        self.detail = None;
        self.texture = None;
        self.measured = None;
        self.measuring = None;
        self.layer = facts.layer_count() as usize - 1;
        self.view = MaskView::default();
        self.source = Some(Source::Read(Box::new(ReadFile {
            fingerprint: file_fingerprint(file),
            path: path.to_owned(),
            facts,
            open,
            plate,
        })));
        Ok(())
    }

    /// What the file being shown says about itself, or `None` while a plate is being shown.
    pub fn read_facts(&self) -> Option<&SlicedFile> {
        match self.source.as_ref()? {
            Source::Read(read) => Some(&read.facts),
            Source::Cut(_) => None,
        }
    }

    /// The file being shown, by the name it was opened under.
    pub fn read_path(&self) -> Option<&Path> {
        match self.source.as_ref()? {
            Source::Read(read) => Some(&read.path),
            Source::Cut(_) => None,
        }
    }

    /// The panel the file was written for, which is what its layers have to be drawn on: a
    /// profile loaded in the window says nothing about a file another slicer wrote.
    pub fn read_panel(&self) -> Option<RasterSettings> {
        let facts = self.read_facts()?;
        let (width_mm, height_mm) = facts.display_mm?;
        Some(RasterSettings {
            width_px: facts.width_px,
            height_px: facts.height_px,
            pitch: PixelPitch {
                x: Scalar::from(width_mm) / facts.width_px as Scalar,
                y: Scalar::from(height_mm) / facts.height_px as Scalar,
            },
            mirror_x: false,
            mirror_y: false,
            shading: Shading::Coverage,
            grey: Grey::default(),
            blur_px: 0,
        })
    }

    /// Closes whatever file is open and leaves the panel empty.
    pub fn close_file(&mut self) {
        if matches!(self.source, Some(Source::Read(_))) {
            *self = Self::default();
        }
    }

    /// Exposure of the layer being shown as the file's own table states it, seconds.
    pub fn read_layer_exposure_s(&self) -> Option<f32> {
        let facts = self.read_facts()?;
        Some(facts.layers.get(self.layer)?.exposure_s)
    }

    /// The file being shown was opened over another plate than the one standing now, so
    /// what the window states about the plate and what the file states are two sources at
    /// once; see `docs/decisions/0151`.
    pub fn file_is_over_an_old_plate(&self, plate: u64) -> bool {
        match self.source.as_ref() {
            Some(Source::Read(read)) => read.plate != plate,
            Some(Source::Cut(_)) | None => false,
        }
    }
}

use std::path::Path;

use core_analysis::Measured;
use core_format::{PrintJob, ReadWriteSeek};
use core_pipeline::{Observer, SlicedFormat, Tolerance, Writing, Written, raster_settings};
use core_raster::RasterSettings;
use core_slicer::{LayerPlan, Solid, Windows};
use core_thumbnail::{Part, ThumbnailSettings, render};

use crate::bake::{bake, parts};
use crate::cut::cut;
use crate::error::EngineError;
use crate::plate::{Model, Plate};

/// A plate ready to be written: baked, with the layers it will be cut in and the header its file
/// carries. Built once, written or measured often; no stack is ever held whole (ADR 0010, 0068).
pub struct Run {
    solid: Solid,
    windows: Windows,
    job: PrintJob,
    panel: RasterSettings,
    format: SlicedFormat,
    fold: Measured,
    raster_window: usize,
}

impl Run {
    /// Bakes `plate`, works out its layers and builds the header its file will carry.
    pub fn of(plate: &Plate) -> Result<Self, EngineError> {
        Self::built(plate, thumbnail(&plate.models))
    }

    /// The same for a plate that is only measured, never written: its header carries no
    /// picture, which costs as much to draw as a coarse stack does to cure.
    pub fn measuring(plate: &Plate) -> Result<Self, EngineError> {
        Self::built(plate, None)
    }

    fn built(
        plate: &Plate,
        thumbnail: Option<core_thumbnail::Thumbnail>,
    ) -> Result<Self, EngineError> {
        let baked =
            bake(&plate.models, &plate.cutting.compensation).ok_or(EngineError::EmptyPlate)?;
        let panel = raster_settings(&plate.printer, plate.panel);
        panel.validate().map_err(EngineError::Panel)?;
        let windows = cut(&baked, &plate.cutting)?;

        let material = plate.material.on(&plate.printer);
        let mut fold = Measured::new(material.bottom_layers as usize);
        if plate.remove_islands {
            fold = fold.removing_hanging(plate.hanging_passes);
        }

        let job = PrintJob {
            printer: plate.printer.clone(),
            material,
            raster: panel,
            plan: windows.plan().clone(),
            // Only known once the stack has been cut; `finish` lays it down again
            // (ADR 0067).
            volume_mm3: 0.0,
            exposure: plate.exposure.clone(),
            thumbnail,
            created_unix_s: plate.created_unix_s,
        };

        Ok(Self {
            solid: baked.solid,
            windows,
            job,
            panel,
            format: plate.format,
            fold,
            raster_window: plate.raster_window,
        })
    }

    /// The whole plate in plate coordinates, which is what gets cut.
    pub fn solid(&self) -> &Solid {
        &self.solid
    }

    /// The stack as it is cut, a window of layers at a time.
    pub fn windows(&self) -> &Windows {
        &self.windows
    }

    /// Where every layer of this run starts and stops.
    pub fn plan(&self) -> &LayerPlan {
        self.windows.plan()
    }

    /// How many layers the plate is cut into.
    pub fn layer_count(&self) -> usize {
        self.windows.layer_count()
    }

    /// The panel the masks are drawn for.
    pub fn panel(&self) -> &RasterSettings {
        &self.panel
    }

    /// Everything the file carries but the masks.
    pub fn job(&self) -> &PrintJob {
        &self.job
    }

    /// Cuts, rasterises and writes the whole stack into `sink`, a window at a time. `name` is the
    /// file name without directory or extension; `None` means cancelled.
    pub fn write(
        &self,
        name: &str,
        sink: &mut dyn ReadWriteSeek,
        observer: &mut dyn Observer,
    ) -> Result<Option<Written>, EngineError> {
        Ok(core_pipeline::write_to(
            &self.writing(name),
            sink,
            observer,
        )?)
    }

    /// The same into a file at `path`, which is removed again when the run fails or is
    /// cancelled. The only thing here that knows what a file is.
    pub fn write_file(
        &self,
        path: &Path,
        observer: &mut dyn Observer,
    ) -> Result<Option<Written>, EngineError> {
        let name = path.file_stem().unwrap_or_default().to_string_lossy();
        Ok(core_pipeline::write(&self.writing(&name), path, observer)?)
    }

    /// Measures what the stack cures without writing it anywhere, so a caller can say
    /// what the print takes before committing to a file.
    pub fn measure(&self, observer: &mut dyn Observer) -> Result<Option<Measured>, EngineError> {
        Ok(core_pipeline::measure(
            &self.solid,
            &self.windows,
            &self.panel,
            &Tolerance::of(&self.job.material),
            self.fold.clone(),
            observer,
        )?)
    }

    fn writing<'a>(&'a self, name: &'a str) -> Writing<'a> {
        Writing {
            format: self.format,
            name,
            job: &self.job,
            source: &self.solid,
            windows: &self.windows,
            settings: &self.panel,
            window: self.raster_window,
            fold: self.fold.clone(),
        }
    }
}

/// The picture of the plate the file carries, or `None` when the plate held nothing with
/// geometry.
fn thumbnail(models: &[Model]) -> Option<core_thumbnail::Thumbnail> {
    let placed = parts(models);
    let parts: Vec<Part<'_>> = placed
        .iter()
        .map(|(mesh, transform)| Part::new(mesh, *transform))
        .collect();
    (!parts.is_empty()).then(|| render(&parts, &ThumbnailSettings::default()))
}

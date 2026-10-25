use std::num::NonZeroU8;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_engine::{Cutting, Plate};
use core_format::{ExposurePlan, ExposureRange};
use core_geometry::Scalar;
use core_raster::{RasterSettings, Shading};
use core_slicer::{AdaptiveSettings, ONE_SAMPLE, WINDOW_LAYERS};
use printer_profiles::{Catalogue, MaterialProfile, PrinterProfile, Renamed};

use crate::i18n::tr;
use crate::job::{Outcome, SliceJob, SliceRequest, SlicedFormat, models_of, worker_threads};
use crate::preview::Fold;
use crate::scene::Scene;
use crate::status::Status;
use core_pipeline::{PanelOverrides, Tolerance, raster_settings};

/// Bounds on the layer height field: ten microns is the finest an MSLA panel is worth
/// slicing in, and above a millimetre no resin cures through.
pub const MIN_LAYER_HEIGHT_MM: Scalar = 0.01;
pub const MAX_LAYER_HEIGHT_MM: Scalar = 1.0;

/// The exposures a change of layer height carried along, as they stood before it, so
/// that the panel can say so and put them back; see `docs/decisions/0128`.
#[derive(Debug, Clone, PartialEq)]
pub struct Rescaled {
    /// The layer height they were for, millimetres.
    pub from_mm: Scalar,
    pub exposure_s: f32,
    pub bands: Vec<ExposureRange>,
}

/// What the window will slice with, and the job it has running.
pub struct Slicing {
    /// The printer the masks are drawn for. Without it there is no panel to draw on.
    pub printer: Option<PrinterProfile>,
    /// The catalogue id of that printer, when it came from the catalogue. It is what a
    /// resin is retuned against.
    pub printer_id: Option<String>,
    /// The resin as the current printer needs it, which is what the file is written with.
    pub material: MaterialProfile,
    /// The resin as it was loaded, tuning tables and all, so a new printer can retune it.
    base_material: MaterialProfile,
    pub resin_id: Option<String>,
    /// Whether a resin was ever chosen. Nothing ships an exposure, so without one the
    /// window refuses to slice rather than printing at invented numbers (ADR 0196).
    chosen_resin: bool,
    /// Every printer and resin the picker offers.
    pub catalogue: Catalogue,
    /// The rules an adaptive stack follows, or `None` for one thickness throughout.
    pub adaptive: Option<AdaptiveSettings>,
    /// Exposure bands over the resin's own, edited in the Slicing panel.
    pub exposure: Vec<ExposureRange>,
    /// How many planes are sampled inside each layer's band.
    pub samples: NonZeroU8,
    pub anti_alias: bool,
    /// How many greys an anti-aliased edge is rounded to, or `None` for all 255.
    pub grey_levels: Option<NonZeroU8>,
    /// Radius an anti-aliased edge is faded over, pixels; `0` leaves it sharp.
    pub blur_px: u8,
    /// Whether every island is taken out of the written file.
    pub remove_islands: bool,
    /// How many layers more of what stood on an island taken out go too, one a pass,
    /// when islands are taken out at all (ADR 0228).
    pub hanging_passes: u32,
    /// What the save dialog offers and what a `.ctb` name is written at.
    pub format: SlicedFormat,
    /// The exposures the last change of layer height carried along, as they were.
    rescaled: Option<Rescaled>,
    pub job: Option<SliceJob>,
    /// The file the last job wrote, until someone takes it. What happens to it next is
    /// not this module's business; see `network`.
    written: Option<Written>,
    /// Whether what the running job writes is a file to be sent on.
    to_send: bool,
    /// How many times a printer or resin has been taken into hand this run; see
    /// `History::observe`.
    chosen: u64,
    /// Plates still to cut, most recent last, for a run over the whole project. One job
    /// runs at a time: the machine's cores go into the layers of one plate, not into two
    /// plates at once.
    queued: Vec<(u32, PathBuf)>,
}

/// A file a job wrote, and what it was written for.
pub struct Written {
    pub path: PathBuf,
    /// Written to be sent on to the machine rather than to a name the user chose.
    pub to_send: bool,
}

impl Default for Slicing {
    fn default() -> Self {
        let material = MaterialProfile::default();
        Self {
            printer: None,
            printer_id: None,
            base_material: material.clone(),
            material,
            resin_id: None,
            chosen_resin: false,
            // The shipped catalogue is pinned by a test in printer-profiles; a build that
            // broke one of its files opens with an empty picker rather than not at all.
            catalogue: Catalogue::bundled().unwrap_or_default(),
            adaptive: None,
            exposure: Vec::new(),
            samples: ONE_SAMPLE,
            anti_alias: true,
            grey_levels: None,
            blur_px: 0,
            remove_islands: false,
            hanging_passes: 0,
            format: SlicedFormat::default(),
            rescaled: None,
            job: None,
            written: None,
            to_send: false,
            chosen: 0,
            queued: Vec::new(),
        }
    }
}

impl Slicing {
    /// Lays the user's own profile directory over the shipped catalogue. A directory
    /// that is not there is normal; one that cannot be read is worth saying out loud.
    /// Answers the profiles moved to ids of their own on the way (ADR 0261).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_user_profiles(&mut self, status: &mut Status) -> Vec<Renamed> {
        let Some(dir) = printer_profiles::user_dir() else {
            return Vec::new();
        };
        let loaded = self
            .catalogue
            .overlay(&dir)
            .and_then(|()| self.catalogue.give_own_ids());
        loaded.unwrap_or_else(|error| {
            *status = Status::failed(&anyhow::Error::new(error).context(tr!(
                "slicing-cannot-read-profiles",
                dir = dir.display().to_string()
            )));
            Vec::new()
        })
    }

    /// Lays the profiles the page keeps over the shipped catalogue; a browser has no
    /// directory to give.
    #[cfg(target_arch = "wasm32")]
    pub fn load_user_profiles(&mut self, status: &mut Status) -> Vec<Renamed> {
        let store = std::sync::Arc::new(crate::web::profiles::PageProfiles);
        let loaded = self
            .catalogue
            .overlay_store(store)
            .and_then(|()| self.catalogue.give_own_ids());
        loaded.unwrap_or_else(|error| {
            *status = Status::failed(
                &anyhow::Error::new(error).context(tr!("slicing-cannot-read-profiles-browser")),
            );
            Vec::new()
        })
    }

    /// The resin as it was loaded, which is what a project file records: the retuned copy
    /// follows from it and the printer.
    pub fn base_material(&self) -> &MaterialProfile {
        &self.base_material
    }

    /// A new resin, as loaded. It is retuned for the printer in hand and brings the layer
    /// height it was measured at with it.
    pub fn set_material(&mut self, material: MaterialProfile) {
        self.base_material = material;
        self.chosen_resin = true;
        self.retune();
    }

    /// A new printer. The resin in hand stays only where it is set up on it, and gives way
    /// to the first that is, or to none: another light engine's exposure is no calibration.
    pub fn set_printer(&mut self, printer: PrinterProfile, id: Option<String>) {
        let output = printer.output.into();
        self.printer = Some(printer);
        self.printer_id = id;
        self.set_format(output);
        self.adopt_default_resin();
        self.retune();
    }

    /// The container to slice into. One the machine in hand cannot be written — a
    /// `.cxdlp` for a name carrying no model code — gives way to the default, so a
    /// profile or a restored session never arms a button that can only fail.
    pub fn set_format(&mut self, format: SlicedFormat) {
        self.format = match self.printer.as_ref() {
            Some(printer) if !format.writable_for(printer) => SlicedFormat::default(),
            _ => format,
        };
    }

    /// No machine in hand, which is what removing the last one leaves. The resin stays:
    /// it is the user's, until the next machine finds it not set up there.
    pub fn clear_printer(&mut self) {
        self.printer = None;
        self.printer_id = None;
        self.format = SlicedFormat::default();
        self.retune();
    }

    /// Whether a resin was chosen. Without one there is no measured exposure to print at.
    pub fn has_resin(&self) -> bool {
        self.chosen_resin
    }

    /// What the resin picker shows: not having one is a state, not a resin called "no
    /// resin".
    pub fn resin_name(&self) -> String {
        match self.chosen_resin {
            true => self.material.name.clone(),
            false => tr!("slicing-select-resin"),
        }
    }

    /// Reads the resin in hand back out of the catalogue after it was edited there, and
    /// trades it for one of the printer's own if it is no longer set up on this printer.
    pub fn reload_resin(&mut self) {
        if let Some(id) = self.resin_id.clone() {
            match self.catalogue.resin(&id) {
                Ok(entry) => self.base_material = entry.profile.clone(),
                // A resin deleted in Settings is no longer the resin in hand, and there
                // is no measured exposure to fall back on (ADR 0196).
                Err(_) => self.drop_resin(),
            }
        }
        self.adopt_default_resin();
        self.retune();
    }

    /// A printer brings its own resin when the one in hand is not set up on it: the first
    /// the user has on it, or none, because numbers measured elsewhere are not a
    /// calibration and a resin nobody added is not theirs to print with (ADR 0158).
    fn adopt_default_resin(&mut self) {
        let set_up = self
            .printer_id
            .as_deref()
            .is_none_or(|printer| self.base_material.is_tuned_for(printer));
        if self.resin_id.is_some() && set_up {
            return;
        }
        let Some(entry) = self.printer_id.as_deref().and_then(|printer| {
            self.catalogue.resins().find(|entry| {
                crate::settings::installed(&entry.source) && entry.profile.is_tuned_for(printer)
            })
        }) else {
            if self.resin_id.is_some() {
                self.drop_resin();
            }
            return;
        };
        self.resin_id = Some(entry.id.clone());
        self.base_material = entry.profile.clone();
        self.chosen_resin = true;
    }

    /// No resin in hand, and no exposure to slice at until one is chosen.
    fn drop_resin(&mut self) {
        self.resin_id = None;
        self.chosen_resin = false;
        self.base_material = MaterialProfile::default();
    }

    /// How many printers and resins have been taken into hand. The undo stack watches it
    /// so that the layer height and exposures a profile brings with it are not taken for
    /// an edit the user made; see `docs/decisions/0192`.
    pub fn chosen(&self) -> u64 {
        self.chosen
    }

    /// The resin this printer needs, out of the resin as it was loaded.
    fn retune(&mut self) {
        self.chosen += 1;
        self.material = match self.printer_id.as_deref() {
            Some(id) => self.base_material.starting_point(id),
            None => self.base_material.clone(),
        };
        self.rescaled = None;
    }

    /// The layer height the stack is cut at, millimetres: the thickest layer, when the
    /// stack is adaptive.
    pub fn layer_height_mm(&self) -> Scalar {
        self.material.layer_height_mm
    }

    /// Cuts at `layer_height_mm` from now on, carrying the exposures along the resin's
    /// working curve so that a layer cures as deep as it did. What they were is kept for
    /// [`Slicing::revert_exposure`].
    pub fn set_layer_height(&mut self, layer_height_mm: Scalar) {
        let from_mm = self.material.layer_height_mm;
        if (layer_height_mm - from_mm).abs() < f32::EPSILON {
            return;
        }
        let before = self.rescaled.take().unwrap_or_else(|| Rescaled {
            from_mm,
            exposure_s: self.material.exposure_s,
            bands: self.exposure.clone(),
        });
        // Back at the height they were set for, they are put back as they were rather
        // than carried there and back.
        if (layer_height_mm - before.from_mm).abs() < f32::EPSILON {
            self.material.layer_height_mm = layer_height_mm;
            self.material.exposure_s = before.exposure_s;
            self.exposure = before.bands;
            return;
        }
        for band in &mut self.exposure {
            band.exposure_s = self.material.exposure_at(band.exposure_s, layer_height_mm);
        }
        self.material = self.material.rescaled_to(layer_height_mm);
        self.rescaled = Some(before);
    }

    /// The bands as they would stand at the height the resin was measured at, which is
    /// what a project file keeps: it records the resin as loaded, not this session's copy.
    pub fn bands_as_measured(&self) -> Vec<ExposureRange> {
        let measured_mm = match self.printer_id.as_deref() {
            Some(id) => self.base_material.starting_point(id).layer_height_mm,
            None => self.base_material.layer_height_mm,
        };
        self.exposure
            .iter()
            .map(|band| ExposureRange {
                exposure_s: self.material.exposure_at(band.exposure_s, measured_mm),
                ..*band
            })
            .collect()
    }

    /// What the last change of layer height carried along, while nobody has touched it.
    pub fn rescaled(&self) -> Option<&Rescaled> {
        self.rescaled.as_ref()
    }

    /// Puts back the exposures the last change of layer height carried along, at the
    /// height it changed to.
    pub fn revert_exposure(&mut self) {
        if let Some(before) = self.rescaled.take() {
            self.material.exposure_s = before.exposure_s;
            self.exposure = before.bands;
        }
    }

    /// An exposure was set by hand, so there is nothing carried along left to point at.
    pub fn exposure_edited(&mut self) {
        self.rescaled = None;
    }

    /// How the stack will be cut: the height, and the adaptive rules over it.
    pub fn cutting(&self) -> Cutting {
        Cutting {
            samples: self.samples,
            layer_height_mm: self.layer_height_mm(),
            adaptive: self.adaptive,
            compensation: self.material.compensation,
            slice_window: WINDOW_LAYERS,
        }
    }

    /// Whether the printer in hand moves the plate to each layer's own Z.
    pub fn printer_reads_variable_height(&self) -> bool {
        self.printer
            .as_ref()
            .is_some_and(|printer| printer.firmware.variable_layer_height)
    }

    /// Whether the printer in hand reads the per-layer tables rather than the header
    /// alone, which is what an exposure band needs.
    pub fn printer_reads_per_layer(&self) -> bool {
        self.printer
            .as_ref()
            .is_some_and(|printer| printer.firmware.per_layer_settings)
    }

    /// Height above which an exposure band takes effect. Everything below is the bottom
    /// block and its transition, which keep the resin's own ramp; see ADR 0090.
    pub fn band_floor_mm(&self) -> Scalar {
        let material = &self.material;
        let held = material.bottom_layers + u32::from(material.transition_layers);
        held as Scalar * self.layer_height_mm()
    }

    /// Whether the resin in hand carries numbers measured for the printer in hand.
    pub fn resin_is_tuned(&self) -> bool {
        self.printer_id
            .as_deref()
            .is_some_and(|id| self.base_material.is_tuned_for(id))
    }

    /// Why the Slice button is greyed out, or `None` when it is not.
    pub fn blocker(&self, scene: &Scene) -> Option<String> {
        if self.printer.is_none() {
            return Some(tr!("slicing-pick-machine"));
        }
        if !self.chosen_resin {
            return Some(tr!("slicing-add-resin"));
        }
        if !scene.has_printable(scene.active_plate()) {
            return Some(tr!("slicing-nothing-visible"));
        }
        if scene.supports_pending(scene.active_plate()) {
            return Some(tr!("slicing-supports-growing"));
        }
        if !self.exposure.is_empty() && !self.printer_reads_per_layer() {
            return Some(tr!("layers-bands-refused"));
        }
        if self.adaptive.is_some() && !self.printer_reads_variable_height() {
            return Some(tr!("slicing-steps-by-header"));
        }
        None
    }

    /// Starts a job cutting `plate` into `output`. Fails when there is nothing on it.
    pub fn start(&mut self, scene: &Scene, plate: u32, output: PathBuf) -> Result<()> {
        let Some(printer) = self.printer.clone() else {
            bail!(tr!("slicing-no-printer"));
        };
        if !self.chosen_resin {
            bail!(tr!("slicing-no-resin"));
        }
        let models = models_of(scene, plate);
        if models.is_empty() {
            bail!(tr!("slicing-nothing-visible-error"));
        }
        if scene.supports_pending(plate) {
            bail!(tr!("slicing-supports-growing-error"));
        }
        // The output name has the last word on the container, and only the profile knows
        // which revision of it the machine reads; see ADR 0047.
        let format = SlicedFormat::of(&output, self.format.ctb_version())
            .with_context(|| tr!("slicing-no-format", path = output.display().to_string()))?
            .at_revision_of(printer.output);

        self.job = Some(SliceJob::spawn(SliceRequest {
            plate: Plate {
                models,
                printer,
                material: self.material.clone(),
                panel: self.overrides(),
                cutting: self.cutting(),
                exposure: ExposurePlan::new(self.exposure.clone()),
                remove_islands: self.remove_islands,
                hanging_passes: self.hanging_passes,
                format,
                raster_window: worker_threads(),
                created_unix_s: now_unix_s(),
            },
            output,
            threads: worker_threads(),
        }));
        Ok(())
    }

    /// How the stack is folded as it is written: what the plate holds, and whether islands
    /// come out.
    pub fn fold(&self) -> Fold {
        Fold {
            held_layers: self.material.bottom_layers as usize,
            remove_islands: self.remove_islands,
            hanging_passes: self.hanging_passes,
            tolerance: Tolerance::of(&self.material),
        }
    }

    /// The panel the masks are drawn for, or `None` without a printer profile.
    pub fn raster_settings(&self) -> Option<RasterSettings> {
        self.printer
            .as_ref()
            .map(|printer| raster_settings(printer, self.overrides()))
    }

    /// What the window sets over the panel the printer profile describes.
    fn overrides(&self) -> PanelOverrides {
        PanelOverrides {
            shading: self.shading(),
            grey_levels: self.grey_levels,
            grey_floor: None,
            blur_px: self.blur_px,
        }
    }

    fn shading(&self) -> Shading {
        if self.anti_alias {
            Shading::Coverage
        } else {
            Shading::Binary
        }
    }

    /// Drains the running job into the stage notice. Returns whether one is still running,
    /// which is what tells the window it has to keep repainting.
    pub fn poll(&mut self, scene: &Scene, status: &mut Status) -> bool {
        let Some(job) = self.job.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return true;
        };

        self.job = None;
        let to_send = std::mem::take(&mut self.to_send);
        if let Outcome::Written { path, .. } = &outcome {
            self.written = Some(Written {
                path: path.clone(),
                to_send,
            });
        }
        let failed = matches!(outcome, Outcome::Failed(_) | Outcome::Cancelled);
        *status = report(outcome, &self.material);
        if failed {
            self.queued.clear();
            return false;
        }
        self.start_queued(scene, status)
    }

    /// Takes the file the last job wrote, once.
    pub fn take_written(&mut self) -> Option<Written> {
        self.written.take()
    }

    /// Whether the running job writes a file to be sent on to the machine.
    pub fn is_sending(&self) -> bool {
        self.job.is_some() && self.to_send
    }

    /// Says the next file written is for the machine rather than for a name the user
    /// chose, which is what decides whether it is sent on and then deleted.
    pub fn send_when_written(&mut self) {
        self.to_send = true;
    }

    /// Cuts every plate that has anything on it, each into its own file beside `output`.
    /// Returns how many were queued, which is zero when the project is empty.
    pub fn start_all(&mut self, scene: &Scene, output: &Path, status: &mut Status) -> usize {
        let printable: Vec<(u32, &str)> = (0..scene.plates().len() as u32)
            .filter(|&plate| scene.has_printable(plate))
            .map(|plate| (plate, scene.plates()[plate as usize].as_str()))
            .collect();
        let mut queued = core_engine::plate_outputs(output, &printable);
        // Popped from the end, so the first plate has to be the last in.
        queued.reverse();
        self.queued = queued;
        let queued = self.queued.len();
        self.start_queued(scene, status);
        queued
    }

    /// Takes the next plate off the queue and starts it. Answers whether one is running.
    fn start_queued(&mut self, scene: &Scene, status: &mut Status) -> bool {
        let Some((plate, output)) = self.queued.pop() else {
            return false;
        };
        if let Err(error) = self.start(scene, plate, output.clone()).with_context(|| {
            tr!(
                "slice-cannot-slice-into",
                path = output.display().to_string()
            )
        }) {
            self.queued.clear();
            *status = Status::failed(&error);
            return false;
        }
        true
    }
}

/// Turns a finished job into the line the stage notice shows.
fn report(outcome: Outcome, material: &MaterialProfile) -> Status {
    match outcome {
        Outcome::Written {
            path,
            layers,
            clipped_layers,
            volume_mm3,
        } => {
            let clipped = if clipped_layers == 0 {
                String::new()
            } else {
                tr!("slicing-clipped", count = clipped_layers)
            };
            let weight_g = volume_mm3 / 1000.0 * material.density_g_cm3;
            let cost = material
                .cost(weight_g, volume_mm3)
                .map(|cost| format!(", {cost:.2} {}", material.details.currency))
                .unwrap_or_default();
            Status::Info(tr!(
                "slicing-wrote",
                path = path.display().to_string(),
                layers = layers,
                volume = format!("{:.1}", volume_mm3 / 1000.0),
                weight = format!("{weight_g:.1}"),
                cost = cost,
                clipped = clipped
            ))
        }
        Outcome::Cancelled => Status::Info(tr!("slicing-cancelled")),
        Outcome::Failed(message) => Status::Error(message),
    }
}

/// The clock the file is stamped with. One that reads before the epoch stamps the epoch:
/// the field is informational and no printer refuses a file over it. `web_time`, because
/// the standard clock panics in a browser.
fn now_unix_s() -> u64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests;

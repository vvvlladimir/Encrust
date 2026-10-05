use std::num::NonZeroU8;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_engine::{Cutting, Plate};
use core_format::{ExposurePlan, ExposureRange};
use core_geometry::Scalar;
use core_raster::{RasterSettings, Shading};
use core_slicer::{AdaptiveSettings, ONE_SAMPLE, WINDOW_LAYERS};
use printer_profiles::{Catalogue, MaterialProfile, PrinterProfile};

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
    /// Whether every island is taken out of the written file, and what stood on it.
    pub remove_islands: bool,
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
            format: SlicedFormat::default(),
            rescaled: None,
            job: None,
            written: None,
            to_send: false,
            queued: Vec::new(),
        }
    }
}

impl Slicing {
    /// Lays the user's own profile directory over the shipped catalogue. A directory
    /// that is not there is normal; one that cannot be read is worth saying out loud.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_user_profiles(&mut self, status: &mut Status) {
        let Some(dir) = printer_profiles::user_dir() else {
            return;
        };
        if let Err(error) = self.catalogue.overlay(&dir) {
            *status = Status::failed(
                &anyhow::Error::new(error)
                    .context(format!("cannot read your profiles in {}", dir.display())),
            );
        }
    }

    /// Lays the profiles the page keeps over the shipped catalogue; a browser has no
    /// directory to give.
    #[cfg(target_arch = "wasm32")]
    pub fn load_user_profiles(&mut self, status: &mut Status) {
        let store = std::sync::Arc::new(crate::web::profiles::PageProfiles);
        if let Err(error) = self.catalogue.overlay_store(store) {
            *status = Status::failed(
                &anyhow::Error::new(error).context("cannot read your profiles in this browser"),
            );
        }
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
        self.retune();
    }

    /// A new printer. The resin in hand is retuned for it, because an exposure measured
    /// on another machine's light engine is not a calibration for this one.
    pub fn set_printer(&mut self, printer: PrinterProfile, id: Option<String>) {
        self.format = printer.output.into();
        self.printer = Some(printer);
        self.printer_id = id;
        self.adopt_default_resin();
        self.retune();
    }

    /// Reads the resin in hand back out of the catalogue after it was edited there, and
    /// trades it for one of the printer's own if it is no longer set up on this printer.
    pub fn reload_resin(&mut self) {
        if let Some(entry) = self
            .resin_id
            .as_deref()
            .and_then(|id| self.catalogue.resin(id).ok())
        {
            self.base_material = entry.profile.clone();
        }
        self.adopt_default_resin();
        self.retune();
    }

    /// A printer brings its own resin when the one in hand is not set up on it: the first
    /// the user has on it, because numbers measured elsewhere are not a calibration, and
    /// a resin nobody added is not theirs to print with (ADR 0158).
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
            return;
        };
        self.resin_id = Some(entry.id.clone());
        self.base_material = entry.profile.clone();
    }

    /// The resin this printer needs, out of the resin as it was loaded.
    fn retune(&mut self) {
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
    pub fn blocker(&self, scene: &Scene) -> Option<&'static str> {
        if self.printer.is_none() {
            return Some("Load a printer profile to know the panel to slice for.");
        }
        if !scene.has_printable(scene.active_plate()) {
            return Some("Nothing visible on the plate to slice.");
        }
        if !self.exposure.is_empty() && !self.printer_reads_per_layer() {
            return Some("This printer reads the header alone, so exposure cannot vary by height.");
        }
        if self.adaptive.is_some() && !self.printer_reads_variable_height() {
            return Some("This printer steps the plate by the header's layer height.");
        }
        None
    }

    /// Starts a job cutting `plate` into `output`. Fails when there is nothing on it.
    pub fn start(&mut self, scene: &Scene, plate: u32, output: PathBuf) -> Result<()> {
        let Some(printer) = self.printer.clone() else {
            bail!("no printer profile is loaded");
        };
        let models = models_of(scene, plate);
        if models.is_empty() {
            bail!("nothing visible on the plate to slice");
        }
        // The output name has the last word on the container, and only the profile knows
        // which revision of it the machine reads; see ADR 0047.
        let format = SlicedFormat::of(&output, self.format.ctb_version())
            .with_context(|| format!("{} names no sliced-file format", output.display()))?
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

    /// Drains the running job into the status bar. Returns whether one is still running,
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

    /// Says the next file written is for the machine rather than for a name the user
    /// chose, which is what decides whether it is sent on and then deleted.
    pub fn send_when_written(&mut self) {
        self.to_send = true;
    }

    /// Cuts every plate that has anything on it, each into its own file beside `output`.
    /// Returns how many were queued, which is zero when the project is empty.
    pub fn start_all(&mut self, scene: &Scene, output: &Path, status: &mut Status) -> usize {
        let mut queued: Vec<(u32, PathBuf)> = Vec::new();
        for plate in 0..scene.plates().len() as u32 {
            if !scene.has_printable(plate) {
                continue;
            }
            let mut path = per_plate_path(output, scene, plate);
            // Two plates may carry one name. Writing both to it would leave only the
            // second, so the loser takes its number instead.
            if queued.iter().any(|(_, taken)| *taken == path) {
                path = numbered(output, plate);
            }
            queued.push((plate, path));
        }
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
        if let Err(error) = self
            .start(scene, plate, output.clone())
            .with_context(|| format!("cannot slice into {}", output.display()))
        {
            self.queued.clear();
            *status = Status::failed(&error);
            return false;
        }
        true
    }
}

/// Where one plate of a whole-project run is written: the name the user typed with the
/// plate's own name on the end, so a project of six plates comes out named rather than
/// numbered.
fn per_plate_path(output: &Path, scene: &Scene, plate: u32) -> PathBuf {
    let name = scene.plates()[plate as usize]
        .replace(' ', "-")
        .to_lowercase();
    beside(output, &name)
}

/// Where a plate goes when its name is already spoken for.
fn numbered(output: &Path, plate: u32) -> PathBuf {
    beside(output, &format!("plate-{}", plate + 1))
}

fn beside(output: &Path, suffix: &str) -> PathBuf {
    let stem = output.file_stem().unwrap_or_default().to_string_lossy();
    let mut path = output.with_file_name(format!("{stem}-{suffix}"));
    if let Some(extension) = output.extension() {
        path.set_extension(extension);
    }
    path
}

/// Turns a finished job into the line the status bar shows.
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
                format!(", {clipped_layers} layers cut off at the edge of the panel")
            };
            let weight_g = volume_mm3 / 1000.0 * material.density_g_cm3;
            let cost = material
                .cost(weight_g, volume_mm3)
                .map(|cost| format!(", {cost:.2} {}", material.details.currency))
                .unwrap_or_default();
            Status::Info(format!(
                "Wrote {} : {layers} layers, {:.1} ml, {weight_g:.1} g{cost}{clipped}",
                path.display(),
                volume_mm3 / 1000.0,
            ))
        }
        Outcome::Cancelled => Status::Info("Slicing cancelled".to_owned()),
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
mod tests {
    use super::*;
    use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
    use format_chitu::CtbVersion;
    use std::path::Path;
    use std::sync::Arc;

    use crate::scene::ImportSummary;

    const PROFILE: &str = r#"
name = "Test panel"
manufacturer = "Test"

[display]
width_px = 64
height_px = 32
width_mm = 12.8
height_mm = 6.4

[build_volume]
x = 12.8
y = 6.4
z = 10.0
"#;

    fn tetrahedron() -> Mesh {
        Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        )
    }

    fn scene_with_a_model() -> Scene {
        let mut scene = Scene::default();
        let mesh = tetrahedron();
        scene.insert(crate::scene::Imported::new(
            "tetra".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));
        scene
    }

    fn with_printer() -> Slicing {
        Slicing {
            printer: Some(
                PrinterProfile::from_toml_str(PROFILE, Path::new("inline.toml"))
                    .expect("the inline profile is valid"),
            ),
            ..Slicing::default()
        }
    }

    /// A resin measured at 0.05 mm for 2.5 s, with one band of 4 s over it.
    fn measured_resin() -> Slicing {
        let mut slicing = Slicing::default();
        slicing.set_material(MaterialProfile {
            layer_height_mm: 0.05,
            exposure_s: 2.5,
            penetration_depth_mm: None,
            ..MaterialProfile::default()
        });
        slicing.exposure = vec![ExposureRange::new(5.0, 10.0, 4.0)];
        slicing
    }

    #[test]
    fn a_new_layer_height_carries_the_exposures_along_and_remembers_them() {
        let mut slicing = measured_resin();
        slicing.set_layer_height(0.1);

        // Jacobs at the assumed 0.1 mm depth: half a depth thicker is exp(0.5) the dose.
        assert!((slicing.material.exposure_s - 4.1218).abs() < 1e-3);
        assert!((slicing.exposure[0].exposure_s - 6.5949).abs() < 1e-3);
        let rescaled = slicing.rescaled().expect("the change is kept to be shown");
        assert!((rescaled.from_mm - 0.05).abs() < 1e-6);
        assert!((rescaled.exposure_s - 2.5).abs() < 1e-6);
        assert!(
            (slicing.material.bottom_exposure_s - MaterialProfile::default().bottom_exposure_s)
                .abs()
                < 1e-6,
            "the bottom block is exposed to stick, not to cure through"
        );
    }

    #[test]
    fn going_back_puts_the_exposures_back_and_keeps_the_new_height() {
        let mut slicing = measured_resin();
        slicing.set_layer_height(0.1);
        slicing.revert_exposure();

        assert!((slicing.layer_height_mm() - 0.1).abs() < 1e-6);
        assert!((slicing.material.exposure_s - 2.5).abs() < 1e-6);
        assert!((slicing.exposure[0].exposure_s - 4.0).abs() < 1e-6);
        assert_eq!(slicing.rescaled(), None, "nothing is left to point at");
    }

    #[test]
    fn two_changes_go_back_to_what_was_set_before_the_first() {
        let mut slicing = measured_resin();
        slicing.set_layer_height(0.1);
        slicing.set_layer_height(0.03);
        let rescaled = slicing.rescaled().expect("still carried");
        assert!((rescaled.exposure_s - 2.5).abs() < 1e-6);
        assert!((slicing.material.exposure_s - 2.0468).abs() < 1e-3);

        slicing.set_layer_height(0.05);
        assert_eq!(slicing.rescaled(), None, "back where it was measured");
        assert_eq!(
            slicing.material.exposure_s, 2.5,
            "exactly, not carried round"
        );
    }

    #[test]
    fn an_exposure_set_by_hand_stops_being_pointed_at() {
        let mut slicing = measured_resin();
        slicing.set_layer_height(0.1);
        slicing.material.exposure_s = 4.2;
        slicing.exposure_edited();
        slicing.revert_exposure();
        assert_eq!(slicing.material.exposure_s, 4.2);
    }

    #[test]
    fn carrying_the_resin_along_writes_the_same_exposures() {
        let before = measured_resin();
        let mut after = measured_resin();
        after.set_layer_height(0.03);

        let printer = PrinterProfile::from_toml_str(PROFILE, Path::new("inline.toml"))
            .expect("the inline profile is valid");
        let job = |slicing: &Slicing| core_format::PrintJob {
            printer: printer.clone(),
            material: slicing.material.clone(),
            raster: raster_settings(&printer, PanelOverrides::default()),
            plan: core_slicer::LayerPlan::of_count(0.03, 400),
            volume_mm3: 0.0,
            exposure: ExposurePlan::new(slicing.exposure.clone()),
            thumbnail: None,
            created_unix_s: 0,
        };
        let (before, after) = (job(&before), job(&after));
        for index in [0, 10, 100, 250, 399] {
            let (was, is) = (
                before.exposure_of_layer_s(index),
                after.exposure_of_layer_s(index),
            );
            assert!(
                (was - is).abs() < 1e-4,
                "layer {index}: {was} s, now {is} s"
            );
        }
    }

    #[test]
    fn a_project_keeps_the_bands_at_the_height_the_resin_was_measured_at() {
        let mut slicing = measured_resin();
        slicing.set_layer_height(0.1);
        let kept = slicing.bands_as_measured();
        assert!(
            (kept[0].exposure_s - 4.0).abs() < 1e-5,
            "got {}",
            kept[0].exposure_s
        );
    }

    #[test]
    fn the_layer_height_follows_the_material() {
        let mut slicing = Slicing::default();
        slicing.set_material(MaterialProfile {
            layer_height_mm: 0.03,
            ..MaterialProfile::default()
        });
        assert!((slicing.layer_height_mm() - 0.03).abs() < 1e-6);
    }

    #[test]
    fn slicing_without_a_printer_profile_is_blocked() {
        let mut slicing = Slicing::default();
        let scene = scene_with_a_model();
        assert!(slicing.blocker(&scene).is_some());
        assert!(slicing.start(&scene, 0, PathBuf::from("out.goo")).is_err());
        assert!(slicing.job.is_none(), "a rejected start runs nothing");
    }

    #[test]
    fn slicing_an_empty_plate_is_blocked() {
        let mut slicing = with_printer();
        let scene = Scene::default();
        assert!(slicing.blocker(&scene).is_some());
        assert!(slicing.start(&scene, 0, PathBuf::from("out.goo")).is_err());
    }

    #[test]
    fn a_name_no_format_claims_is_refused_before_anything_is_written() {
        let mut slicing = with_printer();
        let output =
            std::env::temp_dir().join(format!("encrust-{}-no-format.sliced", std::process::id()));
        assert!(
            slicing
                .start(&scene_with_a_model(), 0, output.clone())
                .is_err()
        );
        assert!(slicing.job.is_none(), "a rejected start runs nothing");
        assert!(!output.exists(), "nor does it leave a file or a PNG stack");
    }

    #[test]
    fn a_hidden_model_is_not_something_to_slice() {
        let slicing = with_printer();
        let mut scene = scene_with_a_model();
        scene.objects_mut()[0].visible = false;
        assert!(slicing.blocker(&scene).is_some());
    }

    #[test]
    fn a_loaded_printer_and_a_visible_model_are_enough() {
        let slicing = with_printer();
        assert_eq!(slicing.blocker(&scene_with_a_model()), None);
    }

    #[test]
    fn exposure_bands_are_blocked_on_a_printer_that_reads_the_header_alone() {
        let mut slicing = with_printer();
        slicing.exposure = vec![ExposureRange::new(0.0, 5.0, 4.0)];
        assert_eq!(slicing.blocker(&scene_with_a_model()), None);

        if let Some(printer) = slicing.printer.as_mut() {
            printer.firmware.per_layer_settings = false;
        }
        assert!(slicing.blocker(&scene_with_a_model()).is_some());
    }

    #[test]
    fn adaptive_layers_are_blocked_on_a_printer_that_steps_by_the_header() {
        let mut slicing = with_printer();
        slicing.adaptive = Some(AdaptiveSettings::default());
        assert!(slicing.blocker(&scene_with_a_model()).is_some());
        assert_eq!(slicing.cutting().adaptive, slicing.adaptive);

        if let Some(printer) = slicing.printer.as_mut() {
            printer.firmware.variable_layer_height = true;
        }
        assert_eq!(slicing.blocker(&scene_with_a_model()), None);
    }

    #[test]
    fn anti_aliasing_decides_the_shading() {
        let mut slicing = Slicing::default();
        assert_eq!(slicing.shading(), Shading::Coverage);
        slicing.anti_alias = false;
        assert_eq!(slicing.shading(), Shading::Binary);
    }

    #[test]
    fn a_finished_job_names_the_file_and_a_failed_one_shows_red() {
        let resin = MaterialProfile::default();
        let written = report(
            Outcome::Written {
                path: PathBuf::from("cube.goo"),
                layers: 40,
                clipped_layers: 0,
                volume_mm3: 2000.0,
            },
            &resin,
        );
        assert!(!written.is_error());
        assert!(written.text().contains("40 layers"));
        assert!(written.text().contains("2.0 ml"));
        assert!(written.text().contains("2.2 g"), "at the resin's 1.1 g/cm3");

        let clipped = report(
            Outcome::Written {
                path: PathBuf::from("cube.goo"),
                layers: 40,
                clipped_layers: 3,
                volume_mm3: 2000.0,
            },
            &resin,
        );
        assert!(clipped.text().contains("cut off"));

        assert!(report(Outcome::Failed("cannot create".to_owned()), &resin).is_error());
        assert!(!report(Outcome::Cancelled, &resin).is_error());
    }

    #[test]
    fn a_priced_resin_puts_what_the_print_cost_in_the_report() {
        let mut resin = MaterialProfile::default();
        resin.details.price = 20.0;
        let written = report(
            Outcome::Written {
                path: PathBuf::from("cube.goo"),
                layers: 40,
                clipped_layers: 0,
                volume_mm3: 50_000.0,
            },
            &resin,
        );
        assert!(written.text().contains("1.10 €"), "55 g at 20 a kilo");
    }

    #[test]
    fn each_plate_is_written_beside_the_name_that_was_typed() {
        let mut scene = Scene::default();
        scene.add_plate();
        scene.rename_plate(1, "Small parts".to_owned());

        let output = PathBuf::from("/tmp/batch.goo");
        assert_eq!(
            per_plate_path(&output, &scene, 0),
            PathBuf::from("/tmp/batch-plate-1.goo")
        );
        assert_eq!(
            per_plate_path(&output, &scene, 1),
            PathBuf::from("/tmp/batch-small-parts.goo"),
            "a renamed plate is named in the file, not numbered"
        );
    }

    #[test]
    fn two_plates_of_one_name_do_not_write_to_one_file() {
        let mut scene = Scene::default();
        scene.add_plate();
        scene.rename_plate(0, "Body".to_owned());
        scene.rename_plate(1, "Body".to_owned());

        let output = PathBuf::from("/tmp/batch.goo");
        assert_eq!(
            per_plate_path(&output, &scene, 0),
            per_plate_path(&output, &scene, 1),
            "the names agree, which is what start_all has to break"
        );
        assert_eq!(
            numbered(&output, 1),
            PathBuf::from("/tmp/batch-plate-2.goo")
        );
    }

    #[test]
    fn a_run_over_the_project_queues_only_the_plates_with_something_on_them() {
        let mut scene = Scene::default();
        scene.add_plate();
        let mut slicing = Slicing::default();
        let mut status = Status::default();

        // No printer, so nothing actually starts; what is under test is what was queued.
        assert_eq!(
            slicing.start_all(&scene, &PathBuf::from("/tmp/out.goo"), &mut status),
            0,
            "an empty project has no plate worth cutting"
        );
    }

    #[test]
    fn polling_without_a_job_reports_nothing_running() {
        let mut slicing = Slicing::default();
        let mut status = Status::default();
        assert!(!slicing.poll(&Scene::default(), &mut status));
        assert_eq!(status, Status::Idle);
    }

    #[test]
    fn a_stack_cut_to_send_is_still_for_sending_when_the_window_asked_every_frame() {
        let scene = scene_with_a_model();
        let mut slicing = with_printer();
        let mut status = Status::default();
        let path = std::env::temp_dir().join("encrust-slicing-to-send.goo");
        slicing.send_when_written();
        slicing
            .start(&scene, 0, path.clone())
            .expect("a loaded printer and a model");
        let written = loop {
            // The window asks for a written file on every frame, the job's first included.
            if let Some(written) = slicing.take_written() {
                break written;
            }
            slicing.poll(&scene, &mut status);
            assert!(!status.is_error(), "{status:?}");
        };
        std::fs::remove_file(&path).ok();
        assert!(written.to_send, "the Send button asked for this stack");
        assert!(
            !slicing.to_send,
            "the next stack goes to a file unless asked again"
        );
    }

    /// The shipped catalogue with its resins taken as the user's own, which is what a
    /// window someone has set a machine up in looks like; see `docs/decisions/0158`.
    fn resins_installed(name: &str) -> Catalogue {
        let dir =
            std::env::temp_dir().join(format!("encrust-slicing-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut catalogue = Catalogue::with_root(&dir).expect("a missing directory reads as empty");
        let resins: Vec<(String, MaterialProfile)> = catalogue
            .resins()
            .map(|entry| (entry.id.clone(), entry.profile.clone()))
            .collect();
        for (id, resin) in resins {
            catalogue
                .save_resin(&id, &resin)
                .expect("the directory is writable");
        }
        catalogue
    }

    #[test]
    fn picking_a_printer_takes_the_format_its_firmware_reads_and_a_resin_measured_on_it() {
        let mut slicing = Slicing {
            catalogue: resins_installed("picking"),
            ..Slicing::default()
        };
        let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
        let chitu = catalogue.printer("elegoo-mars-3-pro").expect("shipped");

        slicing.set_printer(chitu.profile.clone(), Some(chitu.id.clone()));

        assert_eq!(slicing.format, SlicedFormat::Ctb(CtbVersion::V4));
        assert!(slicing.resin_id.is_some(), "a printer brings a resin");
        assert!(
            slicing.resin_is_tuned(),
            "the resin it brings is one measured on it"
        );
    }

    #[test]
    fn a_resin_is_retuned_when_the_printer_under_it_changes() {
        let mut slicing = Slicing::default();
        let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
        let grey = catalogue.resin("standard-grey").expect("shipped");
        slicing.resin_id = Some(grey.id.clone());
        slicing.set_material(grey.profile.clone());

        let saturn = catalogue.printer("elegoo-saturn-4-ultra").expect("shipped");
        slicing.set_printer(saturn.profile.clone(), Some(saturn.id.clone()));
        let fast = slicing.material.exposure_s;

        let mars_three = catalogue.printer("elegoo-mars-3-pro").expect("shipped");
        slicing.set_printer(mars_three.profile.clone(), Some(mars_three.id.clone()));

        assert!(
            slicing.material.exposure_s > fast,
            "an older LED matrix needs longer than a COB engine for the same resin"
        );
        assert_eq!(slicing.resin_id.as_deref(), Some("standard-grey"));
    }

    #[test]
    fn a_printer_the_resin_is_not_set_up_on_brings_its_own() {
        let mut slicing = Slicing {
            catalogue: resins_installed("its-own"),
            ..Slicing::default()
        };
        let mut grey = slicing
            .catalogue
            .resin("standard-grey")
            .expect("shipped")
            .profile
            .clone();
        grey.printers.remove("elegoo-mars-3-pro");
        slicing.resin_id = Some("standard-grey".to_owned());
        slicing.set_material(grey);

        let mars = slicing
            .catalogue
            .printer("elegoo-mars-3-pro")
            .expect("shipped");
        let (profile, id) = (mars.profile.clone(), mars.id.clone());
        slicing.set_printer(profile, Some(id));
        assert!(
            slicing.resin_is_tuned(),
            "a resin of the Mars' own replaces one it has none of"
        );
    }

    #[test]
    fn a_printer_opened_from_a_file_leaves_the_resin_untuned() {
        let mut slicing = Slicing::default();
        let printer = PrinterProfile::from_toml_str(PROFILE, Path::new("inline.toml"))
            .expect("valid profile");

        slicing.set_printer(printer, None);

        assert!(slicing.printer_id.is_none());
        assert!(
            !slicing.resin_is_tuned(),
            "no id, so nothing to tune against"
        );
    }
}

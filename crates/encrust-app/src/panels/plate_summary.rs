use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use core_format::{ExposurePlan, PrintJob};
use core_geometry::{Mesh, Scalar, signed_volume};
use printer_profiles::MaterialProfile;

use crate::panels::Window;
use crate::scene::Scene;
use crate::slicing::Slicing;
use crate::state::Machine;
use crate::ui::{duration, heading, readings};
use crate::workspace::Mode;

/// What stands on the plate, as the summary states it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Tally {
    pub vertices: usize,
    pub triangles: usize,
    /// Resin the models and their supports take, cubic millimetres.
    pub model_mm3: f32,
    pub supports_mm3: f32,
}

/// The figures over the Slice button: the plate's while it is laid out, and the layer's
/// while a stack is being read.
pub fn ui(ui: &mut egui::Ui, window: &Window) {
    if *window.mode == Mode::Preview && window.machine.preview.layer_count() > 0 {
        let layer = window.machine.preview.layer();
        heading(
            ui,
            &format!("Layer {}", layer + 1),
            Some(block(window.machine, layer)),
        );
        let mut rows = layer_readings(window);
        // A file states its own resin and time; ours describe a plate it knows nothing of.
        if window.machine.preview.read_facts().is_none() {
            rows.extend(print_readings(window));
        }
        readings(ui, &rows);
        return;
    }
    heading(ui, "This plate", None);
    let tally = tally(ui.ctx(), &window.doc.scene);
    readings(
        ui,
        &plate_readings(&window.doc.scene, &window.machine.slicing, &tally),
    );
}

fn plate_readings(scene: &Scene, slicing: &Slicing, tally: &Tally) -> Vec<(&'static str, String)> {
    let (height_mm, layers) = stack(scene, slicing);
    // An adaptive stack only knows its count once the model has been planned, so the
    // summary states the two ends of the range the settings allow.
    let count = match layers {
        (most, Some(fewest)) => format!("{fewest} to {most}"),
        (layers, None) => layers.to_string(),
    };
    let mut rows = vec![("Height", format!("{height_mm:.1} mm")), ("Layers", count)];
    if tally.triangles > 0 {
        rows.extend(tally_readings(tally, &slicing.material));
    }
    rows
}

/// What the file the plate is sliced into comes to: its layers, then its resin, price and
/// time once the stack is measured.
pub fn file_readings(ctx: &egui::Context, window: &Window) -> Vec<(&'static str, String)> {
    let tally = tally(ctx, &window.doc.scene);
    let mut rows = plate_readings(&window.doc.scene, &window.machine.slicing, &tally);
    rows.truncate(2);
    rows.extend(print_readings(window));
    rows
}

fn tally_readings(tally: &Tally, material: &MaterialProfile) -> Vec<(&'static str, String)> {
    let mut readings = vec![
        ("Vertices", tally.vertices.to_string()),
        ("Triangles", tally.triangles.to_string()),
        (
            "Model/supports",
            format!(
                "{:.2}/{:.2} ml",
                tally.model_mm3 / 1000.0,
                tally.supports_mm3 / 1000.0
            ),
        ),
    ];
    let price = |volume_mm3: f32| {
        let weight_g = volume_mm3 / 1000.0 * material.density_g_cm3;
        material.cost(weight_g, volume_mm3)
    };
    if let (Some(model), Some(supports)) = (price(tally.model_mm3), price(tally.supports_mm3)) {
        let currency = &material.details.currency;
        readings.push(("Price", format!("{model:.2}/{supports:.2} {currency}")));
    }
    readings
}

/// The height of everything visible on the plate and how many layers it comes to: one
/// count, or the most and the fewest an adaptive stack could take.
fn stack(scene: &Scene, slicing: &Slicing) -> (Scalar, (usize, Option<usize>)) {
    let Some(bounds) = scene.world_bounds() else {
        return (0.0, (0, None));
    };
    // Counted from the plate up, as it is cut: nothing under the plate is.
    let height_mm = (bounds.maxs.z - bounds.mins.z.max(0.0)).max(0.0);
    let count = |height: Scalar| (height_mm / height).ceil().max(0.0) as usize;
    let fewest = slicing
        .adaptive
        .map(|_| count(slicing.layer_height_mm()))
        .filter(|_| slicing.layer_height_mm() > 0.0);
    let thinnest = slicing
        .adaptive
        .map_or(slicing.layer_height_mm(), |adaptive| adaptive.min_height_mm);
    (height_mm, (count(thinnest), fewest))
}

/// The tally of the active plate, counted again only when what stands on it changes: a
/// volume walks every triangle, which no frame should.
pub fn tally(ctx: &egui::Context, scene: &Scene) -> Tally {
    let plate = scene.active_plate();
    let mut hasher = DefaultHasher::new();
    for object in scene.printable(plate) {
        Arc::as_ptr(model(object)).hash(&mut hasher);
        object
            .transform
            .scale
            .to_array()
            .map(f32::to_bits)
            .hash(&mut hasher);
        for supports in object.supports.meshes().unwrap_or_default() {
            Arc::as_ptr(supports).hash(&mut hasher);
        }
    }
    let fingerprint = hasher.finish();
    let id = egui::Id::new("plate-tally");
    if let Some((cached, tally)) = ctx.data(|data| data.get_temp::<(u64, Tally)>(id))
        && cached == fingerprint
    {
        return tally;
    }

    let mut tally = Tally::default();
    for object in scene.printable(plate) {
        let mesh = model(object);
        let scale = object.transform.scale;
        tally.vertices += mesh.vertices.len();
        tally.triangles += mesh.faces.len();
        tally.model_mm3 += signed_volume(mesh).abs() * (scale.x * scale.y * scale.z).abs();
        for supports in object.supports.meshes().unwrap_or_default() {
            tally.supports_mm3 += signed_volume(supports).abs();
        }
    }
    ctx.data_mut(|data| data.insert_temp(id, (fingerprint, tally)));
    tally
}

/// The mesh an object prints as: its shell once hollowed.
fn model(object: &crate::scene::SceneObject) -> &Arc<Mesh> {
    object.hollow.shell().unwrap_or(&object.mesh)
}

fn layer_readings(window: &Window) -> Vec<(&'static str, String)> {
    let preview = &window.machine.preview;
    // A file's own table, where one is open: the window's resin says nothing about a
    // stack somebody else exposed.
    let exposure_s = preview
        .read_layer_exposure_s()
        .unwrap_or_else(|| layer_exposure_s(window.machine, preview.layer()));

    vec![
        (
            "Z height",
            format!("{:.3} mm", preview.layer_z().unwrap_or_default()),
        ),
        ("Exposure", format!("{exposure_s:.2} s")),
        (
            "Cured area",
            format!("{:.0} mm2", preview.layer_area_mm2().unwrap_or_default()),
        ),
    ]
}

/// What the stack comes to in resin, money and time, measured from the masks.
fn print_readings(window: &Window) -> Vec<(&'static str, String)> {
    let material = &window.machine.slicing.material;
    let mut rows = Vec::new();
    match window.measured() {
        Some(measured) => {
            let volume_mm3 = measured.volume_mm3();
            let weight_g = volume_mm3 / 1000.0 * material.density_g_cm3;
            let price = material.cost(weight_g, volume_mm3).map_or_else(
                || "not priced".to_owned(),
                |cost| format!("{cost:.3} {}", material.details.currency),
            );
            rows.extend([
                ("Volume", format!("{:.3} ml", volume_mm3 / 1000.0)),
                ("Weight", format!("{weight_g:.3} g")),
                ("Price", price),
            ]);
        }
        None => {
            let waiting = if window.machine.preview.is_measuring() {
                "measuring"
            } else {
                "-"
            };
            rows.extend(["Volume", "Weight", "Price"].map(|key| (key, waiting.to_owned())));
        }
    }
    if let Some(seconds) = print_time_s(window.machine) {
        rows.push(("Time", duration(seconds)));
    }
    rows
}

/// The stack as the writer would see it, which is what every per-layer reading here is
/// taken from. `None` until a printer is chosen and the stack is planned.
fn job(machine: &Machine) -> Option<PrintJob> {
    let slicing = &machine.slicing;
    Some(PrintJob {
        printer: slicing.printer.clone()?,
        material: slicing.material.clone(),
        raster: slicing.raster_settings()?,
        plan: machine.preview.plan()?.clone(),
        volume_mm3: 0.0,
        exposure: ExposurePlan::new(slicing.exposure.clone()),
        thumbnail: None,
        created_unix_s: 0,
    })
}

/// What the machine takes over the stack: every layer's exposure, waits and travel.
fn print_time_s(machine: &Machine) -> Option<u32> {
    Some(job(machine)?.print_time_s())
}

/// Where `layer` sits in the resin's exposure ramp, which the heading names.
fn block(machine: &Machine, layer: usize) -> &'static str {
    let material = &machine.slicing.material;
    let bottom = material.bottom_layers as usize;
    match layer {
        layer if layer < bottom => "bottom block",
        layer if layer < bottom + material.transition_layers as usize => "transition",
        _ => "body",
    }
}

/// Exposure of `layer`, seconds: the value the writer would give it, and the resin's own
/// ramp before a printer is chosen.
fn layer_exposure_s(machine: &Machine, layer: usize) -> f32 {
    match job(machine) {
        Some(job) => job.exposure_of_layer_s(layer as u32),
        None => machine.slicing.material.exposure_of_layer_s(layer as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::ImportSummary;
    use core_geometry::{Orientation, Transform, Vec3, diagnose};

    fn scene_with_a_model(height_mm: Scalar) -> Scene {
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, height_mm),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        );
        let mut scene = Scene::default();
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

    #[test]
    fn the_summary_states_both_volumes_and_prices_them_only_when_the_resin_has_a_price() {
        let tally = Tally {
            vertices: 22_579,
            triangles: 43_834,
            model_mm3: 6_660.0,
            supports_mm3: 1_520.0,
        };
        let mut resin = MaterialProfile::default();
        let keys = |readings: Vec<(&'static str, String)>| {
            readings.into_iter().map(|(key, _)| key).collect::<Vec<_>>()
        };
        assert_eq!(
            keys(tally_readings(&tally, &resin)),
            ["Vertices", "Triangles", "Model/supports"]
        );

        resin.details.price = 30.0;
        let priced = tally_readings(&tally, &resin);
        assert_eq!(priced[2].1, "6.66/1.52 ml");
        assert_eq!(priced[3].0, "Price");
    }

    #[test]
    fn the_estimate_counts_the_layers_the_model_needs() {
        let mut slicing = Slicing::default();
        slicing.set_layer_height(0.05);
        // One millimetre at 0.05 mm a layer.
        let (height_mm, layers) = stack(&scene_with_a_model(1.0), &slicing);
        assert!((height_mm - 1.0).abs() < 1e-6);
        assert_eq!(layers, (20, None));
    }

    #[test]
    fn an_adaptive_estimate_states_both_ends_of_what_the_settings_allow() {
        let mut slicing = Slicing::default();
        slicing.set_layer_height(0.1);
        slicing.adaptive = Some(core_slicer::AdaptiveSettings {
            min_height_mm: 0.02,
            ..core_slicer::AdaptiveSettings::default()
        });
        // One millimetre is ten layers at the ceiling and fifty at the floor.
        let (_, layers) = stack(&scene_with_a_model(1.0), &slicing);
        assert_eq!(layers, (50, Some(10)));
    }

    #[test]
    fn an_empty_plate_has_nothing_to_estimate() {
        assert_eq!(
            stack(&Scene::default(), &Slicing::default()),
            (0.0, (0, None))
        );
    }

    fn machine_with_a_ramp() -> Machine {
        let mut machine = Machine::default();
        let material = &mut machine.slicing.material;
        material.bottom_layers = 5;
        material.transition_layers = 5;
        material.bottom_exposure_s = 30.0;
        material.exposure_s = 5.2;
        machine
    }

    #[test]
    fn a_transition_layer_reads_its_own_step_of_the_ramp() {
        let machine = machine_with_a_ramp();
        // Six equal steps from 30 s to 5.2 s, so the first transition layer is one down.
        let first = layer_exposure_s(&machine, 5);
        assert!(
            (first - (30.0 - 24.8 / 6.0)).abs() < 1e-3,
            "expected 25.867 s on the first transition layer, got {first}"
        );
        assert_eq!(
            layer_exposure_s(&machine, 4),
            30.0,
            "still the bottom block"
        );
        assert_eq!(layer_exposure_s(&machine, 10), 5.2, "past the ramp");
    }

    #[test]
    fn the_heading_names_the_block_the_layer_is_in() {
        let machine = machine_with_a_ramp();
        assert_eq!(block(&machine, 4), "bottom block");
        assert_eq!(block(&machine, 5), "transition");
        assert_eq!(block(&machine, 10), "body");
    }
}

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use core_format::{ExposurePlan, PrintJob};
use core_geometry::{Mesh, Scalar, signed_volume};
use core_slicer::LayerPlan;
use printer_profiles::MaterialProfile;

use crate::i18n::tr;
use crate::panels::Window;
use crate::preview::stack_fingerprint;
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
            &tr!("summary-layer", layer = layer + 1),
            Some(&block(window.machine, layer)),
        );
        let mut rows = layer_readings(window);
        rows.extend(print_readings(window));
        readings(ui, &rows);
        return;
    }
    heading(ui, &tr!("summary-this-plate"), None);
    let tally = tally(ui.ctx(), &window.doc.scene);
    let mut rows = plate_readings(&window.doc.scene, &window.machine.slicing, &tally);
    if let Some(time) = plate_time(window) {
        rows.push((tr!("summary-time"), time));
    }
    readings(ui, &rows);
}

/// What the machine takes over the plate: as planned once a stack cut from the plate as it
/// stands is held, estimated from the plate's height until then.
fn plate_time(window: &Window) -> Option<String> {
    let fingerprint = stack_fingerprint(&window.doc.scene, window.machine.slicing.cutting());
    if !window.machine.preview.is_stale(fingerprint)
        && let Some(seconds) = print_time_s(window.machine)
    {
        return Some(duration(seconds));
    }
    let value = match estimated_time_s(&window.doc.scene, &window.machine.slicing)? {
        (longest, Some(shortest)) => tr!(
            "summary-layer-range",
            fewest = duration(shortest),
            most = duration(longest)
        ),
        (seconds, None) => duration(seconds),
    };
    Some(tr!("summary-estimate", value = value))
}

/// The time the stack `stack` counts would take, before anything is cut: one figure, or
/// the longest and the shortest an adaptive stack could take.
fn estimated_time_s(scene: &Scene, slicing: &Slicing) -> Option<(u32, Option<u32>)> {
    let (_, (most, fewest)) = stack(scene, slicing);
    if most == 0 {
        return None;
    }
    let time_s = |count: usize, thickness_mm: Scalar| {
        Some(job_over(slicing, LayerPlan::of_count(thickness_mm, count))?.print_time_s())
    };
    let shortest = match fewest {
        Some(fewest) => Some(time_s(fewest, slicing.layer_height_mm())?),
        None => None,
    };
    Some((time_s(most, thinnest_mm(slicing))?, shortest))
}

fn plate_readings(scene: &Scene, slicing: &Slicing, tally: &Tally) -> Vec<(String, String)> {
    let (height_mm, layers) = stack(scene, slicing);
    // An adaptive stack only knows its count once the model has been planned, so the
    // summary states the two ends of the range the settings allow.
    let count = match layers {
        (most, Some(fewest)) => tr!("summary-layer-range", fewest = fewest, most = most),
        (layers, None) => layers.to_string(),
    };
    let mut rows = vec![
        (
            tr!("summary-height"),
            tr!("value-mm", value = format!("{height_mm:.1}")),
        ),
        (tr!("summary-layers"), count),
    ];
    if tally.triangles > 0 {
        rows.extend(tally_readings(tally, &slicing.material));
    }
    rows
}

fn tally_readings(tally: &Tally, material: &MaterialProfile) -> Vec<(String, String)> {
    let mut readings = vec![
        (tr!("summary-vertices"), tally.vertices.to_string()),
        (tr!("summary-triangles"), tally.triangles.to_string()),
        (
            tr!("summary-model-supports"),
            tr!(
                "value-ml",
                value = format!(
                    "{:.2}/{:.2}",
                    tally.model_mm3 / 1000.0,
                    tally.supports_mm3 / 1000.0
                )
            ),
        ),
    ];
    let weight_g = |volume_mm3: f32| volume_mm3 / 1000.0 * material.density_g_cm3;
    readings.push((
        tr!("summary-weight"),
        tr!(
            "value-g",
            value = format!(
                "{:.2}/{:.2}",
                weight_g(tally.model_mm3),
                weight_g(tally.supports_mm3)
            )
        ),
    ));
    let price = |volume_mm3: f32| material.cost(weight_g(volume_mm3), volume_mm3);
    if let (Some(model), Some(supports)) = (price(tally.model_mm3), price(tally.supports_mm3)) {
        let currency = &material.details.currency;
        readings.push((
            tr!("summary-price"),
            format!("{model:.2}/{supports:.2} {currency}"),
        ));
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
    (height_mm, (count(thinnest_mm(slicing)), fewest))
}

/// The thinnest layer the settings allow, millimetres.
fn thinnest_mm(slicing: &Slicing) -> Scalar {
    slicing
        .adaptive
        .map_or(slicing.layer_height_mm(), |adaptive| adaptive.min_height_mm)
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
            .linear()
            .to_cols_array()
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
        let grows = object.transform.linear().determinant().abs();
        tally.vertices += mesh.vertices.len();
        tally.triangles += mesh.faces.len();
        tally.model_mm3 += signed_volume(mesh).abs() * grows;
        for supports in object.supports.meshes().unwrap_or_default() {
            tally.supports_mm3 += signed_volume(supports).abs();
        }
    }
    ctx.data_mut(|data| data.insert_temp(id, (fingerprint, tally)));
    tally
}

/// The mesh an object prints as: its shell once hollowed, its holes taken out.
fn model(object: &crate::scene::SceneObject) -> &Arc<Mesh> {
    object.printed()
}

fn layer_readings(window: &Window) -> Vec<(String, String)> {
    let preview = &window.machine.preview;
    let exposure_s = layer_exposure_s(window.machine, preview.layer());

    vec![
        (
            tr!("summary-z-height"),
            tr!(
                "value-mm",
                value = format!("{:.3}", preview.layer_z().unwrap_or_default())
            ),
        ),
        (
            tr!("summary-exposure"),
            tr!("value-s", value = format!("{exposure_s:.2}")),
        ),
        (
            tr!("summary-cured-area"),
            tr!(
                "value-mm2",
                value = format!("{:.0}", preview.layer_area_mm2().unwrap_or_default())
            ),
        ),
    ]
}

/// What the stack comes to in resin, money and time, measured from the masks.
fn print_readings(window: &Window) -> Vec<(String, String)> {
    let material = &window.machine.slicing.material;
    let mut rows = Vec::new();
    match window.measured() {
        Some(measured) => {
            let volume_mm3 = measured.volume_mm3();
            let weight_g = volume_mm3 / 1000.0 * material.density_g_cm3;
            let price = material.cost(weight_g, volume_mm3).map_or_else(
                || tr!("summary-not-priced"),
                |cost| format!("{cost:.3} {}", material.details.currency),
            );
            rows.extend([
                (
                    tr!("summary-volume"),
                    tr!("value-ml", value = format!("{:.3}", volume_mm3 / 1000.0)),
                ),
                (
                    tr!("summary-weight"),
                    tr!("value-g", value = format!("{weight_g:.3}")),
                ),
                (tr!("summary-price"), price),
            ]);
        }
        None => {
            let waiting = if window.machine.preview.is_measuring() {
                tr!("summary-measuring")
            } else {
                "-".to_owned()
            };
            rows.extend(
                [
                    tr!("summary-volume"),
                    tr!("summary-weight"),
                    tr!("summary-price"),
                ]
                .map(|key| (key, waiting.clone())),
            );
        }
    }
    if let Some(seconds) = print_time_s(window.machine) {
        rows.push((tr!("summary-time"), duration(seconds)));
    }
    rows
}

/// The stack as the writer would see it, which is what every per-layer reading here is
/// taken from. `None` until a printer is chosen and the stack is planned.
fn job(machine: &Machine) -> Option<PrintJob> {
    job_over(&machine.slicing, machine.preview.plan()?.clone())
}

/// The job `slicing` would write over `plan`, or `None` until a printer is chosen.
fn job_over(slicing: &Slicing, plan: LayerPlan) -> Option<PrintJob> {
    let printer = slicing.printer.clone()?;
    Some(PrintJob {
        material: slicing.material.on(&printer),
        printer,
        raster: slicing.raster_settings()?,
        plan,
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
fn block(machine: &Machine, layer: usize) -> String {
    let material = &machine.slicing.material;
    let bottom = material.bottom_layers as usize;
    match layer {
        layer if layer < bottom => tr!("summary-block-bottom"),
        layer if layer < bottom + material.transition_layers as usize => {
            tr!("summary-block-transition")
        }
        _ => tr!("summary-block-body"),
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
mod tests;

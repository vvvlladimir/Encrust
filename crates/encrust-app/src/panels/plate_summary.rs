use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use core_format::{ExposurePlan, PrintJob};
use core_geometry::{Mesh, Scalar, signed_volume};
use core_slicer::LayerPlan;
use printer_profiles::MaterialProfile;

use egui::{Align, Layout, vec2};

use crate::i18n::tr;
use crate::panels::{Window, destination, slice};
use crate::preview::stack_fingerprint;
use crate::scene::Scene;
use crate::slicing::Slicing;
use crate::state::Machine;
use crate::ui::{
    Figure, card_foot, details_fold, duration, figure_grid, heading, icon, notice, readings,
    split_readings, theme,
};
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

/// The plate's card on the stage's foot, under the models: what the plate comes to — or in
/// Preview the layer shown — then the printer its file goes to and the press that makes the
/// file. The card grows upward as Details opens (ADR 0348, 0382).
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    egui::Frame::new().inner_margin(BODY_MARGIN).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if *window.mode == Mode::Preview && window.machine.preview.layer_count() > 0 {
            layer(ui, window);
        } else {
            plate(ui, window);
        }
    });
    card_foot().show(ui, |ui| {
        ui.set_width(ui.available_width());
        destination::ui(ui, window);
        slice::ui(ui, window);
    });
}

/// Room around the card's heading and figures, over its foot.
const BODY_MARGIN: egui::Margin = egui::Margin {
    left: 12,
    right: 12,
    top: 0,
    bottom: 8,
};

/// The card's heading, `aside` at its right.
fn title_row(ui: &mut egui::Ui, title: &str, aside: Option<&str>) {
    let size = vec2(ui.available_width(), theme::SECTION_H);
    let layout = Layout::left_to_right(Align::Center);
    ui.allocate_ui_with_layout(size, layout, |ui| heading(ui, title, aside));
}

/// The plate's time, resin, weight and price, the rest of them under Details, and a word
/// on any model left broken.
fn plate(ui: &mut egui::Ui, window: &Window) {
    let scene = &window.doc.scene;
    let material = &window.machine.slicing.material;
    let name = scene.plates().get(scene.active_plate() as usize);
    title_row(ui, name.map_or("", String::as_str), Some(&material.name));
    let tally = tally(ui.ctx(), scene);
    let split = (tally.triangles > 0).then(|| Split::of(&tally, material));
    let time = split.as_ref().and_then(|_| plate_time(window));
    let cells = plate_figures(time, split.as_ref(), &material.details.currency);
    figure_grid(ui, &figures(&cells));
    if let Some(split) = &split {
        ui.add_space(FIGURES_GAP);
        details_fold(ui, &tr!("summary-details"), None, |ui| {
            details(ui, &tally, split, &material.details.currency);
        });
    }
    broken_verdict(ui, scene);
}

/// Room between the figures and the Details row, points.
const FIGURES_GAP: f32 = 4.0;

/// The four cells of the plate's grid, `—` in each it cannot state yet.
fn plate_figures(
    time: Option<String>,
    split: Option<&Split>,
    currency: &str,
) -> [(String, String, String); 4] {
    let none = || NO_FIGURE.to_owned();
    let (resin, weight) = split.map_or_else(
        || (none(), none()),
        |split| (two_places(total(split.ml)), two_places(total(split.g))),
    );
    let price = split.and_then(|split| split.price.map(total));
    let unit = |unit: String, known: bool| if known { unit } else { String::new() };
    [
        (
            tr!("summary-time"),
            time.unwrap_or_else(none),
            String::new(),
        ),
        (
            tr!("summary-resin"),
            resin,
            unit(tr!("unit-ml"), split.is_some()),
        ),
        (
            tr!("summary-weight"),
            weight,
            unit(tr!("unit-g"), split.is_some()),
        ),
        (
            tr!("summary-price"),
            price.map_or_else(none, two_places),
            unit(currency.to_owned(), price.is_some()),
        ),
    ]
}

/// What a cell states before there is anything to state.
const NO_FIGURE: &str = "—";

fn two_places(value: f32) -> String {
    format!("{value:.2}")
}

/// The grid's cells, borrowed from what `plate_figures` or `layer_figures` built.
fn figures(cells: &[(String, String, String); 4]) -> [Figure<'_>; 4] {
    cells
        .each_ref()
        .map(|(name, value, unit)| Figure { name, value, unit })
}

/// What Details opens: the models' and the supports' share of the resin, the weight and
/// the price, then the mesh's size.
fn details(ui: &mut egui::Ui, tally: &Tally, split: &Split, currency: &str) {
    let with = |value: f32, unit: &str| format!("{value:.2} {unit}");
    let mut rows = vec![
        (
            tr!("summary-resin"),
            with(split.ml[0], &tr!("unit-ml")),
            with(split.ml[1], &tr!("unit-ml")),
        ),
        (
            tr!("summary-weight"),
            with(split.g[0], &tr!("unit-g")),
            with(split.g[1], &tr!("unit-g")),
        ),
    ];
    if let Some([model, supports]) = split.price {
        rows.push((
            tr!("summary-price"),
            with(model, currency),
            with(supports, currency),
        ));
    }
    split_readings(ui, [&tr!("summary-model"), &tr!("summary-supports")], &rows);
    readings(
        ui,
        &[
            (tr!("summary-vertices"), tally.vertices.to_string()),
            (tr!("summary-triangles"), tally.triangles.to_string()),
        ],
    );
}

/// A model left broken is a verdict here rather than a blocker: what is sliced from it may
/// be nothing like the model, and this is the last place to turn back.
fn broken_verdict(ui: &mut egui::Ui, scene: &Scene) {
    let broken = crate::repair::broken_on_the_plate(scene);
    if broken == 0 {
        return;
    }
    ui.add_space(theme::ITEM_GAP);
    notice(
        ui,
        icon::WARNING,
        theme::colors().warn,
        &tr!("summary-broken", count = broken),
        &tr!("summary-broken-why", count = broken),
    );
}

/// What printing the plate takes — time, resin, weight and price — as the Check tool's
/// estimate reads them.
pub fn estimate(ctx: &egui::Context, window: &Window) -> Vec<(String, String)> {
    let tally = tally(ctx, &window.doc.scene);
    let mut rows = tally_readings(&tally, &window.machine.slicing.material);
    if let Some(time) = plate_time(window) {
        rows.insert(0, (tr!("summary-time"), time));
    }
    rows
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

fn tally_readings(tally: &Tally, material: &MaterialProfile) -> Vec<(String, String)> {
    let split = Split::of(tally, material);
    let both = |[model, supports]: [f32; 2]| format!("{model:.2}/{supports:.2}");
    let mut readings = vec![
        (tr!("summary-vertices"), tally.vertices.to_string()),
        (tr!("summary-triangles"), tally.triangles.to_string()),
        (
            tr!("summary-model-supports"),
            tr!("value-ml", value = both(split.ml)),
        ),
        (tr!("summary-weight"), tr!("value-g", value = both(split.g))),
    ];
    if let Some(price) = split.price {
        let currency = &material.details.currency;
        readings.push((tr!("summary-price"), format!("{} {currency}", both(price))));
    }
    readings
}

/// What the plate's resin comes to, the models' first and the supports' second.
struct Split {
    ml: [f32; 2],
    g: [f32; 2],
    /// `None` while the resin has no price.
    price: Option<[f32; 2]>,
}

impl Split {
    fn of(tally: &Tally, material: &MaterialProfile) -> Self {
        let volumes_mm3 = [tally.model_mm3, tally.supports_mm3];
        let g = volumes_mm3.map(|volume_mm3| volume_mm3 / 1000.0 * material.density_g_cm3);
        let [model, supports] = [0, 1].map(|at| material.cost(g[at], volumes_mm3[at]));
        Self {
            ml: volumes_mm3.map(|volume_mm3| volume_mm3 / 1000.0),
            g,
            price: model
                .zip(supports)
                .map(|(model, supports)| [model, supports]),
        }
    }
}

/// The models' and the supports' together.
fn total([model, supports]: [f32; 2]) -> f32 {
    model + supports
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

/// The layer shown in Preview: its exposure, the block of the ramp it is in, the area it
/// cures and the resin that comes to.
fn layer(ui: &mut egui::Ui, window: &Window) {
    title_row(ui, &tr!("summary-layer"), None);
    let cells = layer_figures(window.machine);
    figure_grid(ui, &figures(&cells));
}

fn layer_figures(machine: &Machine) -> [(String, String, String); 4] {
    let preview = &machine.preview;
    let layer = preview.layer();
    let area_mm2 = preview.layer_area_mm2();
    let thickness_mm = preview
        .layer_z()
        .map(|top| top - preview.bottom_of(layer).unwrap_or(0.0));
    let volume_mm3 = area_mm2.zip(thickness_mm).map(|(area, thick)| area * thick);
    let one_place = |value: Option<Scalar>| {
        value.map_or_else(|| NO_FIGURE.to_owned(), |value| format!("{value:.1}"))
    };
    [
        (
            tr!("summary-exposure"),
            two_places(layer_exposure_s(machine, layer)),
            tr!("unit-s"),
        ),
        (tr!("summary-block"), block(machine, layer), String::new()),
        (
            tr!("summary-cured-area"),
            one_place(area_mm2),
            tr!("unit-mm2"),
        ),
        (
            tr!("summary-volume"),
            one_place(volume_mm3),
            tr!("unit-mm3"),
        ),
    ]
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

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use core_geometry::{Mesh, signed_volume};
use egui::{Align, Layout, RichText};
use printer_profiles::MaterialProfile;

use crate::scene::Scene;
use crate::status::Status;
use crate::ui::{icon, theme};

/// What stands on the plate, as the strip states it at its right.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Tally {
    pub vertices: usize,
    pub triangles: usize,
    /// Resin the models and their supports take, cubic millimetres.
    pub model_mm3: f32,
    pub supports_mm3: f32,
}

pub fn ui(ui: &mut egui::Ui, status: &Status, tally: &Tally, material: &MaterialProfile) {
    let (glyph, color) = if status.is_error() {
        (icon::WARNING, theme::colors().danger)
    } else {
        (icon::INFO, theme::colors().text_low)
    };

    // The readings take their room first, so the message is cut at them rather than
    // drawn over them.
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        if tally.triangles > 0 {
            for (key, value) in readings(tally, material).iter().rev() {
                reading(ui, key, value);
            }
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(RichText::new(glyph).font(theme::icon(13.0)).color(color));
            message(ui, status.text(), color);
        });
    });
}

/// The message in what is left of the strip: cut with an ellipsis where it does not fit,
/// and the whole of it on hover, since a cause chain is as long as it is.
fn message(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    let needed = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::small(), color)
        .size()
        .x;
    let cut = needed > ui.available_width();
    let label = egui::Label::new(RichText::new(text).font(theme::small()).color(color)).truncate();
    let response = ui.add(label);
    if cut {
        response.on_hover_text(text);
    }
}

fn reading(ui: &mut egui::Ui, key: &str, value: &str) {
    let colors = theme::colors();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.label(
            RichText::new(key)
                .font(theme::small())
                .color(colors.text_low),
        );
        ui.label(
            RichText::new(value)
                .font(theme::mono(11.0))
                .color(colors.text_mid),
        );
    });
}

fn readings(tally: &Tally, material: &MaterialProfile) -> Vec<(&'static str, String)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strip_states_both_volumes_and_prices_them_only_when_the_resin_has_a_price() {
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
            keys(readings(&tally, &resin)),
            ["Vertices", "Triangles", "Model/supports"]
        );

        resin.details.price = 30.0;
        let priced = readings(&tally, &resin);
        assert_eq!(priced[2].1, "6.66/1.52 ml");
        assert_eq!(priced[3].0, "Price");
    }
}

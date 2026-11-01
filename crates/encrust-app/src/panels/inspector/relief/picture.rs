//! The relief's picture: the picker, its preview and the gallery of built-in ones.

use super::{carried, truncated};
use crate::i18n::tr;
use crate::panels::Window;
use crate::relief::{BUILTINS, Picture};
use crate::state::Doc;
pub(super) use crate::ui::theme::layout::inspector_relief_picture::{GALLERY_GAP, THUMB_PT};
use crate::ui::{
    file_line, icon, picture_square, secondary_button, subheading, swatch, theme, tone,
};
use core_geometry::Vec2;
use std::sync::Arc;

/// Pixels a side of that thumbnail.
pub(super) const THUMB_PX: usize = 64;

/// The texture in hand: what it is, beside a square of it whose click drops every texture
/// there is to choose instead, with the buttons for a picture of one's own.
pub(super) fn picker(ui: &mut egui::Ui, window: &mut Window) {
    let width = ui.available_width();
    let room = width - THUMB_PT - ui.spacing().item_spacing.x;
    let square = ui
        .horizontal(|ui| {
            let left = egui::Layout::top_down(egui::Align::Min);
            ui.allocate_ui_with_layout(egui::vec2(room, THUMB_PT), left, |ui| {
                ui.set_width(room);
                match &window.tools.relief.picture {
                    Some(picture) => named(ui, picture),
                    None => own(ui, window.doc),
                }
            });
            preview(ui, window.tools.relief.picture.as_ref())
        })
        .inner;
    crate::ui::popup_menu(&square).width(width).show(|ui| {
        subheading(ui, &tr!("relief-builtins"));
        gallery(ui, window);
        picture_buttons(ui, window);
    });
}

/// The square the texture in hand is shown in, with the caret that says a click on it
/// opens the others: the picture, or an empty square for the models' own.
pub(super) fn preview(ui: &mut egui::Ui, picture: Option<&Picture>) -> egui::Response {
    let texture = picture.map(|picture| thumbnail(ui.ctx(), picture));
    picture_square(ui, texture.as_ref(), THUMB_PT, &tr!("relief-builtins"))
}

/// The models' own textures as the panel names them, or that there are none.
pub(super) fn own(ui: &mut egui::Ui, doc: &Doc) {
    let colors = theme::colors();
    tone(ui, &tr!("relief-own"), colors.text_high);
    if doc.scene.targets().any(|object| object.mapped.is_some()) {
        carried(ui, doc);
    } else {
        tone(ui, &tr!("relief-own-none"), colors.text_low);
    }
}

/// The picture in hand by its name and its size.
pub(super) fn named(ui: &mut egui::Ui, picture: &Picture) {
    let colors = theme::colors();
    file_line(ui, &truncated(&picture.name));
    let [width, height] = picture.size_px;
    tone(
        ui,
        &tr!("relief-picture-size", width = width, height = height),
        colors.text_low,
    );
}

/// The picture shrunk to a thumbnail, made into a texture once per picture.
pub(super) fn thumbnail(ctx: &egui::Context, picture: &Picture) -> egui::TextureHandle {
    let id = egui::Id::new(("relief-thumbnail", Arc::as_ptr(&picture.heights) as usize));
    if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return texture;
    }
    let pixels = (0..THUMB_PX * THUMB_PX)
        .map(|index| {
            let (x, y) = (index % THUMB_PX, index / THUMB_PX);
            let uv = Vec2::new(
                (x as f32 + 0.5) / THUMB_PX as f32,
                1.0 - (y as f32 + 0.5) / THUMB_PX as f32,
            );
            let grey = (picture.heights.sample(uv).clamp(0.0, 1.0) * 255.0).round() as u8;
            theme::mask_pixel(grey, false)
        })
        .collect();
    let image = egui::ColorImage::new([THUMB_PX, THUMB_PX], pixels);
    let texture = ctx.load_texture("relief-thumbnail", image, egui::TextureOptions::NEAREST);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}

/// Textures in a row of the gallery, and the gap between two, points.
pub(super) const GALLERY_COLUMNS: usize = 4;

/// The textures that come with the window, a click on one taking it as the picture.
pub(super) fn gallery(ui: &mut egui::Ui, window: &mut Window) {
    let picked = window
        .tools
        .relief
        .picture
        .as_ref()
        .and_then(|picture| picture.builtin);
    let side = (ui.available_width() - GALLERY_GAP * (GALLERY_COLUMNS - 1) as f32)
        / GALLERY_COLUMNS as f32;
    let mut clicked = None;
    for (row, builtins) in BUILTINS.chunks(GALLERY_COLUMNS).enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GALLERY_GAP;
            for (column, builtin) in builtins.iter().enumerate() {
                let index = row * GALLERY_COLUMNS + column;
                let Some(texture) = builtin_thumbnail(ui.ctx(), index) else {
                    continue;
                };
                let name = (builtin.name)();
                if swatch(ui, &texture, side, picked == Some(index), &name).clicked() {
                    clicked = Some(index);
                }
            }
        });
        ui.add_space(GALLERY_GAP);
    }
    if let Some(index) = clicked {
        let relief = &mut window.tools.relief;
        crate::relief::choose_builtin(relief, &mut window.machine.status, index);
    }
}

/// One shipped texture shrunk to a thumbnail, made into a texture the first time it is
/// shown. `None` only for one that does not decode, which a test rules out.
pub(super) fn builtin_thumbnail(ctx: &egui::Context, index: usize) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("relief-builtin", index));
    if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    let grey = BUILTINS.get(index)?.thumbnail(THUMB_PX as u32).ok()?;
    let pixels = grey
        .into_iter()
        .map(|grey| theme::mask_pixel(grey, false))
        .collect();
    let image = egui::ColorImage::new([THUMB_PX, THUMB_PX], pixels);
    let texture = ctx.load_texture("relief-builtin", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    Some(texture)
}

/// Choosing a picture, and going back to the models' own textures.
pub(super) fn picture_buttons(ui: &mut egui::Ui, window: &mut Window) {
    let relief = &mut window.tools.relief;
    let label = if relief.picture.is_some() {
        tr!("relief-choose-another")
    } else {
        tr!("relief-choose-picture")
    };
    if secondary_button(ui, icon::OPEN, &label).clicked() {
        crate::relief::open_picture(relief, &mut window.machine.status);
    }
    if relief.picture.is_some()
        && secondary_button(ui, icon::RESET, &tr!("relief-own-textures")).clicked()
    {
        relief.picture = None;
    }
}

//! The start page, shown until the first model, project or sliced file arrives: a drop target
//! over recent files or bundled test prints. See `docs/decisions/0220`, `0232`, `0234`.

use egui::{Align, Layout, Rect, Ui, UiBuilder, pos2, vec2};

use crate::files::{self, Handed, Wanted};
use crate::i18n::tr;
use crate::panels::{Window, unreadable_profiles};
use crate::recent::RecentFile;
use crate::samples::SAMPLES;
use crate::ui::{
    PictureTile, TILE_PICTURE_H, ZONE_INSET, ZoneWords, drop_zone_box, inline_button_w,
    picture_tile, start_title,
};
use crate::ui::{Segment, Segmented, ago, describe, inline_button, quiet_button, theme, tone};
use crate::workspace::Tool;

/// The page's room under the top bar and either side of it.
const PAD_TOP: f32 = 48.0;
const PAD_SIDE: f32 = 140.0;
/// Tiles stand as many to a row as fit at their least width, the gap between two.
const TILE_MIN_W: f32 = 180.0;
const TILE_GAP: f32 = 14.0;
/// The drop zone: its height, its edge's dashes and weight, its corner, and the square of
/// its glyph.
const ZONE_H: f32 = 104.0;
/// The switch between the two shelves.
const SHELF_SWITCH_W: f32 = 220.0;
/// A tile's picture, and the room under it for the name and the two words.
const CAPTION_H: f32 = 52.0;

/// Whether the window is still on its start page. It leaves for good the moment anything
/// is on the plate or under the layer strip, or a tool is picked.
pub fn still_starting(window: &mut Window) -> bool {
    let quiet = window.doc.scene.is_empty()
        && window.doc.project.path.is_none()
        && !window.doc.imports.is_busy()
        && *window.tool == Tool::default();
    if !quiet {
        window.view.options.started = true;
    }
    !window.view.options.started
}

pub fn ui(ui: &mut Ui, window: &mut Window) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let room = ui.available_width();
            let side = PAD_SIDE.min(((room - TILE_MIN_W) / 2.0).max(0.0));
            let width = (room - 2.0 * side).max(0.0);
            let top = ui.max_rect().top() + PAD_TOP;
            let left = ui.max_rect().left() + side;
            let column = Rect::from_min_max(pos2(left, top), pos2(left + width, f32::INFINITY));
            let mut page = ui.new_child(
                UiBuilder::new()
                    .max_rect(column)
                    .layout(Layout::top_down(Align::Min)),
            );
            heading(&mut page, window);
            // The stage's notice is not on this page, so a failure is said here instead.
            if window.machine.status.is_error() {
                page.add_space(8.0);
                tone(
                    &mut page,
                    &window.machine.status.text(),
                    theme::colors().danger,
                );
            }
            if unreadable_profiles::any(ui.ctx(), window) {
                page.add_space(theme::ITEM_GAP);
                unreadable_profiles::row(&mut page, window);
            }
            page.add_space(28.0);
            drop_zone(&mut page, window, vec2(width, ZONE_H));
            page.add_space(theme::PAGE_GAP);
            shelf_head(&mut page, window);
            page.add_space(theme::SECTION_GAP);
            shelf_tiles(&mut page, window, width);
            let used = page.min_rect().bottom() - ui.max_rect().top() + PAD_TOP;
            ui.allocate_space(vec2(ui.available_width(), used));
        });
}

/// The page's title, and under it what a plate opened now is set up with.
fn heading(ui: &mut Ui, window: &Window) {
    let slicing = &window.machine.slicing;
    let setup = tr!(
        "start-setup",
        machine = slicing.printer.as_ref().map_or_else(
            || window.doc.plate.display_name(),
            |profile| profile.name.clone()
        ),
        resin = slicing.resin_name(),
        layer = format!("{:.0}", slicing.layer_height_mm() * 1000.0)
    );
    start_title(ui, &tr!("start-title"), &setup);
}

/// How many tiles stand in a row across `width`.
fn columns(width: f32) -> usize {
    let fit = ((width + TILE_GAP) / (TILE_MIN_W + TILE_GAP)).floor();
    (fit as usize).max(1)
}

/// One tile's width when `columns(width)` of them span `width`.
fn tile_w(width: f32) -> f32 {
    let columns = columns(width) as f32;
    ((width - TILE_GAP * (columns - 1.0)) / columns)
        .floor()
        .max(0.0)
}

/// Where a file dropped on the window goes, said where the user is looking. A click on it
/// asks for a model or a project.
fn drop_zone(ui: &mut Ui, window: &mut Window, size: egui::Vec2) {
    let words = ZoneWords {
        name: tr!("start-zone"),
        title: tr!("start-drop-here"),
        formats: tr!("start-drop-formats"),
    };
    let (response, words_end) = drop_zone_box(ui, size, &words);
    let (model, project) = buttons_row(ui, response.rect, words_end);
    let wanted = match (model, project, response.clicked()) {
        (true, _, _) => Some(Wanted::Model),
        (_, true, _) => Some(Wanted::Project),
        (_, _, true) => Some(Wanted::ModelOrProject),
        _ => None,
    };
    if let Some(file) = wanted.and_then(files::pick) {
        crate::app::open_by_what_it_is(window, file);
    }
}

/// The zone's two buttons, set against its right edge, or none where they would cover the
/// words ending at `words_end`: a click on the zone still asks. Answers which was pressed.
fn buttons_row(ui: &mut Ui, zone: Rect, words_end: f32) -> (bool, bool) {
    let width = |text: &str| inline_button_w(ui, "", text);
    let (open_model, open_project) = (tr!("start-open-model"), tr!("start-open-project"));
    let both = width(&open_model) + width(&open_project) + theme::ITEM_GAP;
    let at = pos2(
        zone.right() - ZONE_INSET - both,
        zone.center().y - theme::BUTTON_H / 2.0,
    );
    if at.x < words_end + 24.0 {
        return (false, false);
    }
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(at, vec2(both, theme::BUTTON_H)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    row.spacing_mut().item_spacing.x = theme::ITEM_GAP;
    let model = inline_button(&mut row, "", &open_model, true, true).clicked();
    let project = inline_button(&mut row, "", &open_project, false, true).clicked();
    (model, project)
}

/// What the tiles under the drop zone show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Shelf {
    #[default]
    Recent,
    TestPrints,
}

impl Shelf {
    fn id() -> egui::Id {
        egui::Id::new("start-shelf")
    }
}

/// The shelf the tiles show, kept for the window's run.
fn shelf(ctx: &egui::Context) -> Shelf {
    ctx.data(|data| data.get_temp(Shelf::id()))
        .unwrap_or_default()
}

/// The switch between the files opened lately and the test prints, and on the first a way
/// to clear them.
fn shelf_head(ui: &mut Ui, window: &mut Window) {
    let mut shown = shelf(ui.ctx());
    ui.horizontal(|ui| {
        let segments = [
            Segment::new(Shelf::Recent, tr!("start-recent")),
            Segment::new(Shelf::TestPrints, tr!("start-test-prints")),
        ];
        if Segmented::new(&segments)
            .width(SHELF_SWITCH_W)
            .show(ui, &mut shown)
        {
            ui.ctx()
                .data_mut(|data| data.insert_temp(Shelf::id(), shown));
        }
        let recent = &mut window.machine.recent;
        if shown == Shelf::Recent && !recent.files().is_empty() {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if quiet_button(ui, "", &tr!("start-clear-recent"), false, true).clicked() {
                    recent.clear();
                }
            });
        }
    });
}

/// The shelf's tiles, as many to a row as fit.
fn shelf_tiles(ui: &mut Ui, window: &mut Window, width: f32) {
    let size = vec2(tile_w(width), TILE_PICTURE_H + CAPTION_H);
    let columns = columns(width);
    match shelf(ui.ctx()) {
        Shelf::Recent => recent(ui, window, size, columns),
        Shelf::TestPrints => test_prints(ui, window, size, columns),
    }
}

/// `count` tiles in rows of `columns`, each drawn by `add`. Answers the index clicked.
fn grid(
    ui: &mut Ui,
    count: usize,
    columns: usize,
    mut add: impl FnMut(&mut Ui, usize) -> egui::Response,
) -> Option<usize> {
    let mut picked = None;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = vec2(TILE_GAP, TILE_GAP);
        for start in (0..count).step_by(columns) {
            ui.horizontal_top(|ui| {
                for index in start..count.min(start + columns) {
                    if add(ui, index).clicked() {
                        picked = Some(index);
                    }
                }
            });
        }
    });
    picked
}

/// The files opened lately as tiles of what they held, each opened again by a click.
fn recent(ui: &mut Ui, window: &mut Window, size: egui::Vec2, columns: usize) {
    let files = window.machine.recent.files().to_vec();
    if files.is_empty() {
        describe(ui, &tr!("start-nothing-recent"));
        return;
    }
    let now = crate::updates::now_s();
    let picked = grid(ui, files.len(), columns, |ui, index| {
        let file = &files[index];
        let name = file
            .path
            .file_stem()
            .map_or_else(|| file.name(), |stem| stem.to_string_lossy().into_owned());
        let shown = PictureTile {
            name: &name,
            left: ago(Some(file.opened_at_s), now),
            right: kind(file),
            picture: picture(ui.ctx(), &file.path),
        };
        picture_tile(ui, &shown, size).on_hover_text(file.path.display().to_string())
    });
    if let Some(index) = picked {
        open_recent(window, files[index].path.clone());
    }
}

/// The bundled test prints as tiles, each put on the plate by a click.
fn test_prints(ui: &mut Ui, window: &mut Window, size: egui::Vec2, columns: usize) {
    let picked = grid(ui, SAMPLES.len(), columns, |ui, index| {
        let sample = &SAMPLES[index];
        let name = sample.name();
        let shown = PictureTile {
            name: &name,
            left: tr!("start-test-print"),
            right: "STL".to_owned(),
            picture: Some(sample_picture(ui.ctx(), index)),
        };
        picture_tile(ui, &shown, size).on_hover_text(sample.about())
    });
    if let Some(index) = picked {
        crate::app::open_by_what_it_is(window, SAMPLES[index].handed());
    }
}

/// Opens a file from the recent list again, or drops it from the list and says so if it has
/// gone since.
pub fn open_recent(window: &mut Window, path: std::path::PathBuf) {
    if path.exists() {
        crate::app::open_by_what_it_is(window, Handed::Path(path));
    } else {
        window.machine.recent.forget(&path);
        window.machine.status =
            crate::status::Status::Error(tr!("start-gone", path = path.display().to_string()));
    }
}

/// What kind of file it is, as a tile names it: "Project", or the model's extension.
fn kind(file: &RecentFile) -> String {
    match file.path.extension().and_then(|ext| ext.to_str()) {
        Some(core_engine::project::EXTENSION) => tr!("start-project"),
        Some(ext) => ext.to_uppercase(),
        None => String::new(),
    }
}

/// The file's picture as a texture, read from disk once a window and theme. A file without
/// one is remembered as having none, so the folder is not asked again every frame.
fn picture(ctx: &egui::Context, path: &std::path::Path) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("recent-picture", path, ctx.theme()));
    if let Some(known) = ctx.data(|data| data.get_temp::<Option<egui::TextureHandle>>(id)) {
        return known;
    }
    let texture = crate::recent::picture::load(path).map(|image| {
        let name = format!("recent {}", path.display());
        ctx.load_texture(name, image, egui::TextureOptions::LINEAR)
    });
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}

/// The test print's picture as a texture, rendered from its mesh once a window and theme.
fn sample_picture(ctx: &egui::Context, index: usize) -> egui::TextureHandle {
    let id = egui::Id::new(("sample-picture", index, ctx.theme()));
    if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return texture;
    }
    let sample = &SAMPLES[index];
    let parts = vec![(
        std::sync::Arc::new(sample.mesh()),
        core_geometry::Transform::default(),
    )];
    let image = crate::recent::picture::rendered(&parts);
    let texture = ctx.load_texture(sample.name(), image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_holds_as_many_tiles_to_a_row_as_fit_at_their_least() {
        assert_eq!(columns(1000.0), 5, "1280 less 140 either side");
        let width = 2.0 * TILE_MIN_W + TILE_GAP + 10.0;
        assert_eq!(columns(width), 2);
        assert!(tile_w(width) >= TILE_MIN_W);
    }

    #[test]
    fn a_page_narrower_than_one_tile_still_holds_one() {
        assert_eq!(columns(40.0), 1);
        assert_eq!(columns(0.0), 1);
        assert_eq!(tile_w(0.0), 0.0);
    }
}

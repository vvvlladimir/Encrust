//! The start page, shown until the first model, project or sliced file arrives: a drop target
//! over recent files or bundled test prints. See `docs/decisions/0220`, `0232`, `0234`.

use egui::{Align, Align2, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, pos2, vec2};

use crate::files::{self, Handed, Wanted};
use crate::i18n::tr;
use crate::panels::Window;
use crate::recent::RecentFile;
use crate::samples::SAMPLES;
use crate::ui::{ago, describe, focus_ring, icon, inline_button, theme, tone};
use crate::workspace::Tool;

/// The page's column, and the recent files' tiles across it with the gap between two: as
/// many as fit at their least width, never more than `COLUMNS`.
const COLUMN_W: f32 = 980.0;
const COLUMNS: usize = 4;
const TILE_MIN_W: f32 = 160.0;
const TILE_GAP: f32 = 16.0;
/// The height of the drop zone and of the switch beside it.
const ACTIONS_H: f32 = 112.0;
/// The dashes of the zone's edge.
const DASH: f32 = 4.0;
/// The room around a tile's picture, and the height under it for the name and the age.
const TILE_PAD: f32 = 8.0;
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
            let width = COLUMN_W.min(ui.available_width() - 4.0 * TILE_GAP).max(0.0);
            let top = ui.max_rect().top() + 40.0;
            let left = ui.max_rect().center().x - width / 2.0;
            let column = Rect::from_min_max(pos2(left, top), pos2(left + width, f32::INFINITY));
            let mut page = ui.new_child(
                UiBuilder::new()
                    .max_rect(column)
                    .layout(Layout::top_down(Align::Min)),
            );
            heading(&mut page);
            // The stage's notice is not on this page, so a failure is said here instead.
            if window.machine.status.is_error() {
                page.add_space(8.0);
                tone(
                    &mut page,
                    &window.machine.status.text(),
                    theme::colors().danger,
                );
            }
            page.add_space(28.0);
            actions(&mut page, window, width);
            page.add_space(36.0);
            shelf_tiles(&mut page, window, width);
            let used = page.min_rect().bottom() - ui.max_rect().top() + 48.0;
            ui.allocate_space(vec2(ui.available_width(), used));
        });
}

fn heading(ui: &mut Ui) {
    let colors = theme::colors();
    ui.label(
        RichText::new(tr!("start-title"))
            .font(theme::page_title(26.0))
            .color(colors.text_high),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(tr!("start-subtitle"))
            .font(theme::lede())
            .color(colors.text_mid),
    );
}

/// How many tiles stand in a row across `width`.
fn columns(width: f32) -> usize {
    let fit = ((width + TILE_GAP) / (TILE_MIN_W + TILE_GAP)).floor();
    (fit as usize).clamp(1, COLUMNS)
}

/// One tile's width when `columns(width)` of them span `width`.
fn tile_w(width: f32) -> f32 {
    let columns = columns(width) as f32;
    ((width - TILE_GAP * (columns - 1.0)) / columns)
        .floor()
        .max(0.0)
}

/// The drop zone across every column but the last, the switch between the shelves in it,
/// or the two one over the other when a row holds one tile.
fn actions(ui: &mut Ui, window: &mut Window, width: f32) {
    if columns(width) == 1 {
        drop_zone(ui, window, vec2(width, ACTIONS_H));
        ui.add_space(TILE_GAP);
        switch(ui, window, vec2(width, ACTIONS_H));
        return;
    }
    let folder_w = tile_w(width);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = TILE_GAP;
        drop_zone(
            ui,
            window,
            vec2((width - folder_w - TILE_GAP).max(0.0), ACTIONS_H),
        );
        switch(ui, window, vec2(folder_w, ACTIONS_H));
    });
}

/// Where a file dropped on the window goes, said where the user is looking. A click on it
/// asks for a model or a project.
fn drop_zone(ui: &mut Ui, window: &mut Window, size: egui::Vec2) {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tr!("start-zone"))
    });
    // The edge's stroke straddles the rect, so the clip is a point wider.
    let painter = ui.painter_at(rect.expand(1.0));
    let words_end = paint_zone(&painter, rect, response.hovered());
    focus_ring(ui, &response, theme::R_PANEL);
    let (model, project) = buttons_row(ui, rect, words_end);
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

/// The zone's dashed edge and what it says on its left, lit in the accent under the pointer.
/// Answers where the words end.
fn paint_zone(painter: &egui::Painter, rect: Rect, hovered: bool) -> f32 {
    let colors = theme::colors();
    let (fill, edge, glyph) = match hovered {
        true => (colors.accent_wash, colors.accent_deep, colors.accent_soft),
        false => (colors.base, colors.line, colors.text_low),
    };
    painter.rect_filled(rect, theme::R_PANEL, fill);
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    let edge = Stroke::new(1.0, edge);
    painter.extend(egui::Shape::dashed_line(&corners, edge, DASH, DASH));
    let left = rect.left_center();
    let lines = [
        (
            vec2(40.0, 0.0),
            icon::DROP_HERE.to_owned(),
            theme::icon(26.0),
            glyph,
        ),
        (
            vec2(72.0, -11.0),
            tr!("start-drop-here"),
            theme::page_title(17.0),
            colors.text_high,
        ),
        (
            vec2(72.0, 13.0),
            tr!("start-drop-formats"),
            theme::small(),
            colors.text_low,
        ),
    ];
    let mut words_end = left.x;
    for (offset, text, font, tint) in lines {
        let align = match offset.x < 60.0 {
            true => Align2::CENTER_CENTER,
            false => Align2::LEFT_CENTER,
        };
        words_end = words_end.max(painter.text(left + offset, align, text, font, tint).right());
    }
    words_end
}

/// The zone's two buttons, set against its right edge, or none where they would cover the
/// words ending at `words_end`: a click on the zone still asks. Answers which was pressed.
fn buttons_row(ui: &mut Ui, zone: Rect, words_end: f32) -> (bool, bool) {
    let colors = theme::colors();
    let width = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), theme::label(), colors.text_high)
            .size()
            .x
            + 32.0
    };
    let (open_model, open_project) = (tr!("start-open-model"), tr!("start-open-project"));
    let both = width(&open_model) + width(&open_project) + theme::ITEM_GAP;
    let at = pos2(
        zone.right() - 24.0 - both,
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

    fn other(self) -> Self {
        match self {
            Self::Recent => Self::TestPrints,
            Self::TestPrints => Self::Recent,
        }
    }

    fn title(self) -> String {
        match self {
            Self::Recent => tr!("start-recent"),
            Self::TestPrints => tr!("start-test-prints"),
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Self::Recent => icon::RECENT,
            Self::TestPrints => icon::TEST_PRINTS,
        }
    }
}

/// The shelf the tiles show, kept for the window's run.
fn shelf(ctx: &egui::Context) -> Shelf {
    ctx.data(|data| data.get_temp(Shelf::id()))
        .unwrap_or_default()
}

/// The card beside the drop zone: it names the shelf not shown, and a click shows it.
fn switch(ui: &mut Ui, window: &Window, size: egui::Vec2) {
    let colors = theme::colors();
    let to = shelf(ui.ctx()).other();
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, to.title()));
    let (fill, edge, glyph) = match response.hovered() {
        true => (colors.raised, colors.accent_deep, colors.accent_soft),
        false => (colors.base, colors.line, colors.text_mid),
    };
    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        theme::R_PANEL,
        fill,
        Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink(18.0);
    let count = match to {
        Shelf::Recent => window.machine.recent.files().len(),
        Shelf::TestPrints => SAMPLES.len(),
    };
    let count = match (to, count) {
        (Shelf::Recent, count) => tr!("start-files", count = count),
        (Shelf::TestPrints, count) => tr!("start-models", count = count),
    };
    let title = to.title();
    let marks = [
        (inner.left_top(), to.glyph(), theme::icon(22.0), glyph),
        (
            inner.left_bottom() - vec2(0.0, 18.0),
            &title,
            theme::block_title(),
            colors.text_high,
        ),
        (inner.left_bottom(), &count, theme::small(), colors.text_low),
    ];
    for (at, text, font, tint) in marks {
        let align = match at.y < inner.center().y {
            true => Align2::LEFT_TOP,
            false => Align2::LEFT_BOTTOM,
        };
        painter.text(at, align, text, font, tint);
    }
    focus_ring(ui, &response, theme::R_PANEL);
    if response.clicked() {
        ui.ctx().data_mut(|data| data.insert_temp(Shelf::id(), to));
    }
}

/// The shelf's title, and its tiles as many to a row as fit.
fn shelf_tiles(ui: &mut Ui, window: &mut Window, width: f32) {
    let colors = theme::colors();
    let shown = shelf(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(shown.glyph())
                .font(theme::icon(15.0))
                .color(colors.text_low),
        );
        ui.label(
            RichText::new(shown.title())
                .font(theme::block_title())
                .color(colors.text_high),
        );
    });
    ui.add_space(12.0);
    let tile = tile_w(width);
    let picture_h = ((tile - 2.0 * TILE_PAD) * 2.0 / 3.0).round().max(0.0);
    let size = vec2(tile, TILE_PAD + picture_h + CAPTION_H);
    let columns = columns(width);
    match shown {
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
        let shown = Tile {
            name: &name,
            left: ago(Some(file.opened_at_s), now),
            right: kind(file),
            picture: picture(ui.ctx(), &file.path),
        };
        tile(ui, &shown, size).on_hover_text(file.path.display().to_string())
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
        let shown = Tile {
            name: &name,
            left: tr!("start-test-print"),
            right: "STL".to_owned(),
            picture: Some(sample_picture(ui.ctx(), index)),
        };
        tile(ui, &shown, size).on_hover_text(sample.about())
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

/// What one tile shows: a picture, or a glyph when there is none, over a name and two
/// words under it.
struct Tile<'a> {
    name: &'a str,
    left: String,
    right: String,
    picture: Option<egui::TextureHandle>,
}

fn tile(ui: &mut Ui, shown: &Tile<'_>, size: egui::Vec2) -> egui::Response {
    let colors = theme::colors();
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, shown.name));
    let (fill, edge) = match response.hovered() {
        true => (colors.raised, colors.accent_deep),
        false => (colors.base, colors.line),
    };
    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        theme::R_PANEL,
        fill,
        Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink(TILE_PAD);
    let stage = Rect::from_min_size(
        inner.min,
        vec2(inner.width(), size.y - TILE_PAD - CAPTION_H),
    );
    match &shown.picture {
        Some(texture) => {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            let shape =
                egui::epaint::RectShape::filled(stage, theme::R_CONTROL, egui::Color32::WHITE)
                    .with_texture(texture.id(), uv);
            painter.add(shape);
        }
        None => {
            painter.rect_filled(stage, theme::R_CONTROL, colors.sunken);
            let glyph = theme::icon(28.0);
            painter.text(
                stage.center(),
                Align2::CENTER_CENTER,
                icon::MODEL,
                glyph,
                colors.text_low,
            );
        }
    }
    caption(
        &painter,
        shown,
        inner.shrink2(vec2(4.0, 0.0)),
        stage.bottom(),
    );
    focus_ring(ui, &response, theme::R_PANEL);
    response
}

/// The name on one line, cut short to fit, and the two words under it at either end.
fn caption(painter: &egui::Painter, shown: &Tile<'_>, inner: Rect, top: f32) {
    let colors = theme::colors();
    let mut job = egui::text::LayoutJob::simple_singleline(
        shown.name.to_owned(),
        theme::label(),
        colors.text_high,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(inner.width());
    let galley = painter.layout_job(job);
    painter.galley(pos2(inner.left(), top + 10.0), galley, colors.text_high);
    let second = top + 38.0;
    for (x, align, text) in [
        (inner.left(), Align2::LEFT_CENTER, &shown.left),
        (inner.right(), Align2::RIGHT_CENTER, &shown.right),
    ] {
        painter.text(
            pos2(x, second),
            align,
            text,
            theme::small(),
            colors.text_low,
        );
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

/// The file's picture as a texture, read from disk once a window. A file without one is
/// remembered as having none, so the folder is not asked again every frame.
fn picture(ctx: &egui::Context, path: &std::path::Path) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("recent-picture", path));
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

/// The test print's picture as a texture, rendered from its mesh once a window.
fn sample_picture(ctx: &egui::Context, index: usize) -> egui::TextureHandle {
    let id = egui::Id::new(("sample-picture", index));
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
    fn a_wide_page_holds_four_tiles_to_a_row() {
        assert_eq!(columns(COLUMN_W), 4);
        assert_eq!(columns(4000.0), COLUMNS, "never more than COLUMNS");
    }

    #[test]
    fn a_narrow_page_holds_fewer_tiles_no_narrower_than_the_least() {
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

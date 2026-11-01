//! The start page, shown until the first model, project or sliced file arrives: a drop target
//! over recent files or bundled test prints. See `docs/decisions/0220`, `0232`, `0234`.

use egui::{Align, Align2, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, pos2, vec2};

use crate::files::{self, Handed, Wanted};
use crate::i18n::tr;
use crate::panels::{Window, unreadable_profiles};
use crate::recent::RecentFile;
use crate::samples::SAMPLES;
use crate::ui::{
    Segment, Segmented, ago, describe, focus_ring, icon, inline_button, quiet_button, theme, tone,
};
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
const DASH: f32 = 4.0;
const ZONE_EDGE_W: f32 = 1.5;
const ZONE_R: f32 = 12.0;
const ZONE_SQUARE: f32 = 52.0;
/// The switch between the two shelves.
const SHELF_SWITCH_W: f32 = 220.0;
/// A tile's picture, and the room under it for the name and the two words.
const PICTURE_H: f32 = 120.0;
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
            page.add_space(34.0);
            shelf_head(&mut page, window);
            page.add_space(14.0);
            shelf_tiles(&mut page, window, width);
            let used = page.min_rect().bottom() - ui.max_rect().top() + PAD_TOP;
            ui.allocate_space(vec2(ui.available_width(), used));
        });
}

/// The page's title, and under it what a plate opened now is set up with.
fn heading(ui: &mut Ui, window: &Window) {
    let colors = theme::colors();
    ui.label(
        RichText::new(tr!("start-title"))
            .font(theme::page_title(26.0))
            .color(colors.text_high),
    );
    ui.add_space(4.0);
    let slicing = &window.machine.slicing;
    let setup = tr!(
        "start-setup",
        machine = window.doc.plate.display_name(),
        resin = slicing.resin_name(),
        layer = format!("{:.0}", slicing.layer_height_mm() * 1000.0)
    );
    ui.label(
        RichText::new(setup)
            .font(theme::lede())
            .color(colors.text_mid),
    );
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
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tr!("start-zone"))
    });
    // The edge's stroke straddles the rect, so the clip is a point wider.
    let painter = ui.painter_at(rect.expand(ZONE_EDGE_W));
    let words_end = paint_zone(&painter, rect, response.hovered());
    focus_ring(ui, &response, ZONE_R as u8);
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

/// The zone's dashed, rounded edge, the glyph on its square and the words beside it, the
/// edge lit in the accent under the pointer. Answers where the words end.
fn paint_zone(painter: &egui::Painter, rect: Rect, hovered: bool) -> f32 {
    let colors = theme::colors();
    let edge = match hovered {
        true => colors.accent,
        false => colors.line,
    };
    painter.rect_filled(rect, ZONE_R, colors.base);
    let outline = rounded_outline(rect, ZONE_R);
    let stroke = Stroke::new(ZONE_EDGE_W, edge);
    painter.extend(egui::Shape::dashed_line(&outline, stroke, DASH, DASH));
    let square = Rect::from_min_size(
        pos2(rect.left() + 28.0, rect.center().y - ZONE_SQUARE / 2.0),
        vec2(ZONE_SQUARE, ZONE_SQUARE),
    );
    painter.rect_filled(square, ZONE_R, colors.accent_wash);
    painter.text(
        square.center(),
        Align2::CENTER_CENTER,
        icon::DROP_HERE,
        theme::icon(24.0),
        colors.accent_soft,
    );
    let words_at = pos2(square.right() + 18.0, rect.center().y);
    let lines = [
        (
            vec2(0.0, -10.0),
            tr!("start-drop-here"),
            theme::page_title(16.0),
            colors.text_high,
        ),
        (
            vec2(0.0, 12.0),
            tr!("start-drop-formats"),
            theme::small(),
            colors.text_low,
        ),
    ];
    let mut words_end = square.right();
    for (offset, text, font, tint) in lines {
        let drawn = painter.text(words_at + offset, Align2::LEFT_CENTER, text, font, tint);
        words_end = words_end.max(drawn.right());
    }
    words_end
}

/// The points round `rect` with its corners rounded at `radius`, closed, for a dashed edge.
fn rounded_outline(rect: Rect, radius: f32) -> Vec<egui::Pos2> {
    const STEPS: usize = 6;
    let radius = radius
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0)
        .max(0.0);
    let corners = [
        (pos2(rect.right() - radius, rect.top() + radius), -90.0_f32),
        (pos2(rect.right() - radius, rect.bottom() - radius), 0.0),
        (pos2(rect.left() + radius, rect.bottom() - radius), 90.0),
        (pos2(rect.left() + radius, rect.top() + radius), 180.0),
    ];
    let mut points: Vec<egui::Pos2> = corners
        .iter()
        .flat_map(|(centre, from_deg)| {
            (0..=STEPS).map(move |step| {
                let angle = (from_deg + 90.0 * step as f32 / STEPS as f32).to_radians();
                *centre + radius * vec2(angle.cos(), angle.sin())
            })
        })
        .collect();
    if let Some(&first) = points.first() {
        points.push(first);
    }
    points
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
        zone.right() - 28.0 - both,
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
    let size = vec2(tile_w(width), PICTURE_H + CAPTION_H);
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
    let edge = match response.hovered() {
        true => colors.line,
        false => colors.hairline,
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, theme::R_PANEL, colors.panel);
    let stage = rect.with_max_y(rect.top() + PICTURE_H);
    let top_corners = egui::CornerRadius {
        nw: theme::R_PANEL.nw,
        ne: theme::R_PANEL.ne,
        ..egui::CornerRadius::ZERO
    };
    painter.rect_filled(stage, top_corners, colors.sunken);
    match &shown.picture {
        Some(texture) => {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            let fitted = fit(texture.size_vec2(), stage);
            let shape = egui::epaint::RectShape::filled(fitted, 0.0, egui::Color32::WHITE)
                .with_texture(texture.id(), uv);
            painter.add(shape);
        }
        None => {
            painter.text(
                stage.center(),
                Align2::CENTER_CENTER,
                icon::MODEL,
                theme::icon(28.0),
                colors.text_low,
            );
        }
    }
    painter.rect_stroke(
        rect,
        theme::R_PANEL,
        Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    caption(
        &painter,
        shown,
        rect.shrink2(vec2(12.0, 0.0)),
        stage.bottom(),
    );
    focus_ring(ui, &response, theme::R_PANEL);
    response
}

/// The largest rect of `size`'s shape centred in `room`.
fn fit(size: egui::Vec2, room: Rect) -> Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return room;
    }
    let scale = (room.width() / size.x).min(room.height() / size.y);
    Rect::from_center_size(room.center(), size * scale)
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

    #[test]
    fn a_picture_keeps_its_shape_inside_the_tile() {
        let room = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 120.0));
        let fitted = fit(vec2(300.0, 300.0), room);
        assert_eq!(
            fitted.size(),
            vec2(120.0, 120.0),
            "a square stands 120 tall"
        );
        assert_eq!(fitted.center(), room.center());
    }

    #[test]
    fn a_rounded_outline_closes_on_itself_inside_the_rect() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 40.0));
        let outline = rounded_outline(rect, ZONE_R);
        assert_eq!(outline.first(), outline.last());
        assert!(
            outline
                .iter()
                .all(|point| rect.expand(0.01).contains(*point))
        );
    }
}

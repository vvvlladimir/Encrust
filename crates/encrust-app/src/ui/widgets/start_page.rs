//! The start page's own parts: its title, the zone a file is dropped on, and a tile of a
//! picture over a name; see `docs/design/ui-design-system.md`, Start page.

use egui::{Align2, Rect, Response, Sense, Stroke, Ui, pos2, vec2};

use super::{focus_ring, hover_mix};
use crate::ui::{icon, theme};

const DASH: f32 = 4.0;
const ZONE_EDGE_W: f32 = 1.5;
const ZONE_R: f32 = 12.0;
const ZONE_SQUARE: f32 = 52.0;
pub const TILE_PICTURE_H: f32 = 120.0;

/// How far the zone's square and its buttons stand in from its ends, points.
pub const ZONE_INSET: f32 = 28.0;

/// The start page's title, and the lede under it.
pub fn start_title(ui: &mut Ui, title: &str, lede: &str) {
    let colors = theme::colors();
    let title = egui::RichText::new(title)
        .font(theme::page_title(26.0))
        .color(colors.text_high);
    ui.label(title);
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(lede)
            .font(theme::lede())
            .color(colors.text_mid),
    );
}

/// What the drop zone says: its name read out, its title, and the formats it takes.
pub struct ZoneWords {
    pub name: String,
    pub title: String,
    pub formats: String,
}

/// The zone a file is dropped on, `size`, its dashed edge lit under the pointer. Answers
/// the zone and where its words end, for the buttons beside them.
pub fn drop_zone_box(ui: &mut Ui, size: egui::Vec2, words: &ZoneWords) -> (Response, f32) {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &words.name));
    // The edge's stroke straddles the rect, so the clip is a point wider.
    let painter = ui.painter_at(rect.expand(ZONE_EDGE_W));
    let edge = hover_mix(ui, &response, theme::colors().line, theme::colors().accent);
    let words_end = paint_zone(&painter, rect, edge, words);
    focus_ring(ui, &response, ZONE_R as u8);
    (response, words_end)
}

/// The zone's dashed, rounded edge in `edge`, the glyph on its square and the words beside
/// it. Answers where the words end.
fn paint_zone(painter: &egui::Painter, rect: Rect, edge: egui::Color32, words: &ZoneWords) -> f32 {
    let colors = theme::colors();
    painter.rect_filled(rect, ZONE_R, colors.base);
    let outline = rounded_outline(rect, ZONE_R);
    let stroke = Stroke::new(ZONE_EDGE_W, edge);
    painter.extend(egui::Shape::dashed_line(&outline, stroke, DASH, DASH));
    let square = Rect::from_min_size(
        pos2(
            rect.left() + ZONE_INSET,
            rect.center().y - ZONE_SQUARE / 2.0,
        ),
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
            words.title.clone(),
            theme::page_title(16.0),
            colors.text_high,
        ),
        (
            vec2(0.0, 12.0),
            words.formats.clone(),
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

/// What one tile shows: a picture, or a glyph when there is none, over a name and two
/// words under it.
pub struct PictureTile<'a> {
    pub name: &'a str,
    pub left: String,
    pub right: String,
    pub picture: Option<egui::TextureHandle>,
}

/// Draws `shown` as a tile of `size`, ringed in `line` under the pointer.
pub fn picture_tile(ui: &mut Ui, shown: &PictureTile<'_>, size: egui::Vec2) -> egui::Response {
    let colors = theme::colors();
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, shown.name));
    let edge = hover_mix(ui, &response, colors.hairline, colors.line);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, theme::R_PANEL, colors.panel);
    let stage = rect.with_max_y(rect.top() + TILE_PICTURE_H);
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
fn caption(painter: &egui::Painter, shown: &PictureTile<'_>, inner: Rect, top: f32) {
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

#[cfg(test)]
mod tests {
    use egui::{Rect, pos2, vec2};

    use super::{ZONE_R, fit, rounded_outline};

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

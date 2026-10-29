use super::playback::{fraction, height_range, step};
use super::reading::Reading;
use super::{
    HANDLE, PROFILE_HALF_W, PROFILE_PITCH, RISK, RISK_FIRST, RISK_RUN_GAP, RULE_W, TICK_INSET,
    TICK_W, TICKS,
};
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::inspector::risk_tint;
use crate::preview::stack_fingerprint;
use crate::ui::{focus_ring, theme};
use crate::workspace::Mode;
use egui::{Rect, Sense, Stroke, pos2, vec2};

/// The track itself, its top the top of the print: a press or a drag moves whichever
/// handle is nearer the pointer, the top cut or the floor, to stand under it.
pub(super) fn track(ui: &mut egui::Ui, window: &mut Window, height: f32, reading: &Reading) {
    // An id of its own rather than the next in line: a button appearing before the track
    // mid-drag would otherwise hand the drag to another widget.
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let sense = if reading.at.is_some() {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, egui::Id::new("layer-track"), sense);
    let span = rect.y_range().shrink(HANDLE.y / 2.0);
    if let Some(at) = reading.at {
        drag_handle(ui, window, &response, span, (at, reading.floor));
    }
    if response.has_focus() {
        step_by_arrows(ui, window, response.id);
    }
    let after = Reading::of(window);
    paint_track(ui, window, rect, span, (after.at, after.floor));
    let (layer, layers) = (after.layer, reading.layers);
    response.widget_info(|| {
        let name = tr!("section-slider", count = layers);
        egui::WidgetInfo::slider(after.at.is_some(), layer as f64, name)
    });
    focus_ring(ui, &response, theme::R_CONTROL);
    match (after.at, after.floor) {
        (None, _) => response.on_hover_text(tr!("section-nothing-to-cut")),
        (Some(_), Some((_, first))) => response
            .on_hover_cursor(egui::CursorIcon::ResizeVertical)
            .on_hover_text(tr!("section-layers-range", first = first, layer = layer)),
        (Some(_), None) => response.on_hover_cursor(egui::CursorIcon::ResizeVertical),
    };
}

/// Moves the handle the press took hold of, kept for the length of the drag so the two
/// never swap when they pass each other.
pub(super) fn drag_handle(
    ui: &egui::Ui,
    window: &mut Window,
    response: &egui::Response,
    span: egui::Rangef,
    (at, floor): (f32, Option<(f32, usize)>),
) {
    let held = response.id.with("held");
    let Some(pointer) = response.interact_pointer_pos() else {
        ui.data_mut(|data| data.remove::<bool>(held));
        return;
    };
    let floor_at = floor.map_or(0.0, |(floor_at, _)| floor_at);
    let on_floor = ui
        .data(|data| data.get_temp(held))
        .unwrap_or_else(|| grabs_floor(pointer.y, up(span, at), up(span, floor_at)));
    ui.data_mut(|data| data.insert_temp(held, on_floor));
    let to = span_fraction(span, pointer.y);
    if on_floor {
        scrub_floor(window, to);
    } else {
        scrub(window, to);
    }
}

/// Whether a press at `pointer_y` takes the floor rather than the top: the nearer of the
/// two, and with both under it the floor only from below.
pub(super) fn grabs_floor(pointer_y: f32, top_y: f32, floor_y: f32) -> bool {
    let (to_top, to_floor) = ((pointer_y - top_y).abs(), (pointer_y - floor_y).abs());
    to_floor < to_top || (to_floor == to_top && pointer_y > floor_y)
}

/// The bare arrows step the track that holds the focus, rather than moving the focus off
/// it; with Shift they are the table's ten layers either way.
pub(super) fn step_by_arrows(ui: &egui::Ui, window: &mut Window, id: egui::Id) {
    let arrows = egui::EventFilter {
        vertical_arrows: true,
        ..Default::default()
    };
    ui.memory_mut(|memory| memory.set_focus_lock_filter(id, arrows));
    let (up, down) = ui.input_mut(|input| {
        let mut bare = |key| input.count_and_consume_key(egui::Modifiers::NONE, key);
        (bare(egui::Key::ArrowUp), bare(egui::Key::ArrowDown))
    });
    let layers = up as i64 - down as i64;
    if layers != 0 {
        step(window, layers);
    }
}

/// Moves the cut to `at` of the way up, and stops whatever was playing.
pub(super) fn scrub(window: &mut Window, at: f32) {
    match *window.mode {
        Mode::Preview => {
            let preview = &mut window.machine.preview;
            let last = preview.layer_count().saturating_sub(1);
            preview.set_layer((at * last as f32).round() as usize);
            preview.set_playing(false);
        }
        Mode::Prepare => {
            let Some((bottom, top)) = height_range(window.doc) else {
                return;
            };
            let height = bottom + at * (top - bottom);
            // A strip parked at the top cuts nothing away, and saying so with `None` keeps
            // the section shading off as well as the plane.
            window.view.section.height_mm = (height < top).then_some(height);
            window.view.section.playing = false;
        }
    }
}

/// Moves the floor to `at` of the way up, never past the top cut; the bottom of the track
/// cuts nothing away.
pub(super) fn scrub_floor(window: &mut Window, at: f32) {
    match *window.mode {
        Mode::Preview => {
            let preview = &window.machine.preview;
            let last = preview.layer_count().saturating_sub(1);
            let first = ((at * last as f32).round() as usize).min(preview.layer());
            window.view.section.floor_mm = preview.bottom_of(first);
        }
        Mode::Prepare => {
            let Some((bottom, top)) = height_range(window.doc) else {
                return;
            };
            let height = bottom + at * (top - bottom);
            let ceiling = window.view.section.height_mm.unwrap_or(top);
            window.view.section.floor_mm = (height > bottom).then_some(height.min(ceiling));
        }
    }
}

/// Where along `span` the pointer at `y` stands, from zero at its bottom to one at its top.
pub(super) fn span_fraction(span: egui::Rangef, y: f32) -> f32 {
    if span.span() <= 0.0 {
        return 1.0;
    }
    ((span.max - y) / span.span()).clamp(0.0, 1.0)
}

/// How far down `span` a share `at` of the way up stands.
pub(super) fn up(span: egui::Rangef, at: f32) -> f32 {
    egui::lerp(span.max..=span.min, at)
}

/// The rule, the cured-area profile behind it, the scale at the right, the fill between
/// the two thumbs, the risks across it and the thumbs themselves, in that order.
pub(super) fn paint_track(
    ui: &egui::Ui,
    window: &Window,
    rect: Rect,
    span: egui::Rangef,
    (at, floor): (Option<f32>, Option<(f32, usize)>),
) {
    let colors = theme::colors();
    let painter = ui.painter();
    let rule = Rect::from_x_y_ranges(
        egui::Rangef::new(
            rect.center().x - RULE_W / 2.0,
            rect.center().x + RULE_W / 2.0,
        ),
        span.expand(HANDLE.y / 2.0),
    );
    let round = egui::CornerRadius::same(u8::MAX);

    let rows = (span.span() / PROFILE_PITCH).max(1.0) as usize;
    let marks = marked(window, span).map(|(measured, along)| (Marks::of(measured, rows), along));
    if let Some((marks, along)) = &marks {
        marks.paint_profile(painter, *along, rule);
    }
    painter.rect_filled(rule, round, colors.active);
    // With nothing to cut through the rule stands alone: a scale would measure nothing.
    let Some(at) = at else {
        if let Some((marks, along)) = &marks {
            marks.paint_risks(painter, *along, rule);
        }
        return;
    };
    let ticks_x = egui::Rangef::new(
        rect.right() - TICK_INSET - TICK_W,
        rect.right() - TICK_INSET,
    );
    for tick in 0..TICKS {
        let y = egui::lerp(span, tick as f32 / (TICKS - 1) as f32);
        painter.hline(ticks_x, y, Stroke::new(1.0, colors.line));
    }
    let y = up(span, at);
    let floor_y = floor.map_or(up(span, 0.0), |(floor_at, _)| up(span, floor_at));
    let filled = rule.with_min_y(y).with_max_y(floor_y.max(y));
    painter.rect_filled(filled, round, colors.accent);
    if let Some((marks, along)) = &marks {
        marks.paint_risks(painter, *along, rule);
    }
    for y in [floor_y, y] {
        let handle = Rect::from_center_size(pos2(rect.center().x, y), HANDLE);
        painter.rect(
            handle,
            egui::CornerRadius::same(4),
            colors.text_high,
            Stroke::new(2.0, colors.accent),
            egui::StrokeKind::Outside,
        );
    }
}

/// The stack the marks are read from and the stretch of track it covers: all of it in
/// Preview, the stack's own height within the model's in Prepare.
pub(super) fn marked<'a>(
    window: &'a Window,
    span: egui::Rangef,
) -> Option<(&'a core_analysis::Measured, egui::Rangef)> {
    let measured = window.measured()?;
    if *window.mode == Mode::Preview {
        return Some((measured, span));
    }
    let preview = &window.machine.preview;
    let fingerprint = stack_fingerprint(&window.doc.scene, window.machine.slicing.cutting());
    if preview.is_stale(fingerprint) {
        return None;
    }
    let (bottom, top) = height_range(window.doc)?;
    let stack_top = preview.stack_top_mm()?;
    let from = up(span, fraction(bottom, top, 0.0));
    let to = up(span, fraction(bottom, top, stack_top));
    Some((measured, egui::Rangef::new(to, from)))
}

/// What the strip shows of the stack along its track: the cured area up the print, as a
/// profile either side of the rule, and a bar at every layer named as a risk.
pub(super) struct Marks {
    /// The widest layer in each row of the track, from the plate up, over the widest of
    /// all.
    pub(super) widths: Vec<f32>,
    /// Where each risk sits up the stack, from zero to one, and its colour, lowest first.
    pub(super) risks: Vec<(f32, egui::Color32)>,
}

impl Marks {
    pub(super) fn of(measured: &core_analysis::Measured, rows: usize) -> Self {
        let layers = measured.layers();
        let rows = rows.max(1);
        let mut widths = vec![0.0f32; rows];
        for (index, layer) in layers.iter().enumerate() {
            let row = index * rows / layers.len().max(1);
            widths[row] = widths[row].max(layer.area_mm2);
        }
        let widest = widths.iter().copied().fold(0.0, f32::max);
        if widest > 0.0 {
            widths.iter_mut().for_each(|width| *width /= widest);
        }
        let last = layers.len().saturating_sub(1).max(1) as f32;
        let mut risks: Vec<_> = measured
            .risks()
            .iter()
            .map(|risk| (risk.layer as f32 / last, risk_tint(risk)))
            .collect();
        risks.sort_by(|a, b| a.0.total_cmp(&b.0));
        Self { widths, risks }
    }

    /// A dashed line a row, centred on the rule and as wide as the row's widest layer.
    pub(super) fn paint_profile(&self, painter: &egui::Painter, span: egui::Rangef, rule: Rect) {
        let ink = theme::colors().text_low.gamma_multiply(PROFILE_ALPHA);
        let rows = self.widths.len() as f32;
        let mut profile = Vec::new();
        for (row, width) in self.widths.iter().enumerate() {
            if *width <= 0.0 {
                continue;
            }
            let y = up(span, (row as f32 + 0.5) / rows);
            let half = RULE_W / 2.0 + width * (PROFILE_HALF_W - RULE_W / 2.0);
            let line = [
                pos2(rule.center().x - half, y),
                pos2(rule.center().x + half, y),
            ];
            profile.extend(egui::Shape::dashed_line(
                &line,
                Stroke::new(1.0, ink),
                2.0,
                2.0,
            ));
        }
        painter.extend(profile);
    }

    /// Over the rule and its fill, so a risk under the cut reads as well as one above it;
    /// the first of a run of one colour stands larger.
    pub(super) fn paint_risks(&self, painter: &egui::Painter, span: egui::Rangef, rule: Rect) {
        for (index, (at, tint)) in self.risks.iter().enumerate() {
            let y = up(span, *at);
            let size = if self.starts_run(index, span) {
                RISK_FIRST
            } else {
                RISK
            };
            let bar = Rect::from_center_size(pos2(rule.center().x, y), size);
            painter.rect_filled(bar, egui::CornerRadius::same(1), *tint);
        }
    }

    /// Whether risk `index` opens a run: no risk of its colour stands within
    /// `RISK_RUN_GAP` points under it on a track along `span`.
    pub(super) fn starts_run(&self, index: usize, span: egui::Rangef) -> bool {
        let (at, tint) = self.risks[index];
        let gap = RISK_RUN_GAP / span.span().max(1.0);
        !self.risks[..index]
            .iter()
            .rev()
            .take_while(|(below, _)| at - below <= gap)
            .any(|(_, other)| *other == tint)
    }
}

/// How much of `text_low` the cured-area profile keeps.
const PROFILE_ALPHA: f32 = 0.55;

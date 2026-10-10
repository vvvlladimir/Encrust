use core_geometry::Scalar;
use egui::{Align, Align2, Color32, Layout, Rect, Sense, Stroke, pos2, vec2};

use crate::panels::Window;
use crate::panels::inspector::risk_tint;
use crate::preview::{Preview, stack_fingerprint};
use crate::shortcuts::{self, Action};
use crate::state::Doc;
use crate::ui::{icon, icon_button, theme};
use crate::workspace::{Mode, Section};

/// How many layers a second the play button steps through. Fast enough to read the shape
/// of a print, slow enough to see a layer.
const PLAY_LAYERS_PER_S: f32 = 30.0;

/// One of the three views, stacked; the readout under them; the least room the typed
/// layer is given, so an emptied box still takes a click; and the room between groups.
const VIEW_H: f32 = 30.0;
const READOUT_H: f32 = 52.0;
const LAYER_FIELD_MIN_W: f32 = 8.0;
const GAP: f32 = 10.0;

/// The track: how wide it is, how thick its rule, the band of exposures down its left,
/// and the handle on it.
const TRACK_W: f32 = 40.0;
const RULE_W: f32 = 5.0;
const BAND_W: f32 = 3.0;
const HANDLE: egui::Vec2 = vec2(20.0, 10.0);
/// How many ticks the scale beside the rule carries, and how wide the widest row of the
/// cured-area profile stands either side of the rule.
const TICKS: usize = 18;
const PROFILE_W: f32 = 9.0;

/// Where the viewport cuts the plate's contents this frame, plate millimetres, or `None`
/// when the whole model is drawn.
///
/// The Preview mode always cuts at the layer it is showing, because that is what a layer
/// preview is; the Prepare mode cuts only once the strip has been moved off the top.
pub fn cut_height(mode: Mode, section: &Section, preview: &Preview) -> Option<Scalar> {
    match mode {
        Mode::Preview => preview.layer_z(),
        Mode::Prepare => section.height_mm,
    }
}

/// The strip down the stage's right edge, in every view: which view, where the cut stands,
/// a layer up, the track that moves the cut with the top of the print at its top, a layer
/// down, and play. See `docs/decisions/0217`, `0221`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        views(ui, window);
        ui.add_space(GAP);
        let readout = Reading::of(window);
        if let Some(layer) = readout.show(ui) {
            go_to_layer(window, layer);
        }
        ui.add_space(GAP);
        let movable = readout.at.is_some();
        ui.add_enabled_ui(movable, |ui| step_button(ui, window, 1));
        let below = 2.0 * theme::ICON_SIZE + GAP + 3.0 * ui.spacing().item_spacing.y;
        let track_h = (ui.available_height() - below).max(0.0);
        track(ui, window, track_h, &readout);
        ui.add_enabled_ui(movable, |ui| {
            step_button(ui, window, -1);
            ui.add_space(GAP);
            play_button(ui, window);
        });
    });
}

/// The three ways of looking at the plate: the model, the model and its layer side by
/// side, and the layer alone. The two that read the layers are the Preview mode.
fn views(ui: &mut egui::Ui, window: &mut Window) {
    let colors = theme::colors();
    let showing = match (*window.mode, window.view.options.mask_only) {
        (Mode::Prepare, _) => 0,
        (Mode::Preview, false) => 1,
        (Mode::Preview, true) => 2,
    };
    let toggle = shortcuts::text(Action::ToggleMode);
    let views = [
        (icon::MODEL, "Model", toggle.as_str()),
        (
            icon::SIDE_BY_SIDE,
            "Model and layer side by side",
            toggle.as_str(),
        ),
        (icon::MASK, "Layer mask", ""),
    ];
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, VIEW_H * 3.0), Sense::hover());
    let mut picked = None;
    for (index, (glyph, name, keys)) in views.iter().enumerate() {
        let cell = Rect::from_min_size(
            rect.min + vec2(0.0, VIEW_H * index as f32),
            vec2(width, VIEW_H),
        );
        let response = ui.interact(cell, ui.id().with(("view", index)), Sense::click());
        let on = index == showing;
        let (fill, tint) = match (on, response.hovered()) {
            (true, _) => (colors.accent_wash, colors.accent_soft),
            (false, true) => (colors.hover, colors.text_high),
            (false, false) => (Color32::TRANSPARENT, colors.text_low),
        };
        ui.painter().rect_filled(cell, theme::R_CONTROL, fill);
        ui.painter().text(
            cell.center(),
            Align2::CENTER_CENTER,
            *glyph,
            theme::icon(14.0),
            tint,
        );
        if index > 0 {
            ui.painter().hline(
                cell.x_range(),
                cell.top(),
                Stroke::new(1.0, colors.hairline),
            );
        }
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, *name));
        if response.on_hover_text(format!("{name}  {keys}")).clicked() {
            picked = Some(index);
        }
    }
    ui.painter().rect_stroke(
        rect,
        theme::R_CONTROL,
        Stroke::new(1.0, colors.hairline),
        egui::StrokeKind::Inside,
    );
    match picked {
        Some(0) => *window.mode = Mode::Prepare,
        Some(index) => {
            *window.mode = Mode::Preview;
            window.view.options.mask_only = index == 2;
        }
        None => {}
    }
}

/// A layer up over the track, or a layer down under it.
fn step_button(ui: &mut egui::Ui, window: &mut Window, layers: i64) {
    let glyph = if layers > 0 {
        icon::LAYER_UP
    } else {
        icon::LAYER_DOWN
    };
    if icon_button(ui, glyph, &shortcuts::tooltip(Action::Step(layers))).clicked() {
        step(window, layers);
    }
}

/// Runs the cut up the print, or stops it. Moving the cut back to the top shows the whole
/// model.
fn play_button(ui: &mut egui::Ui, window: &mut Window) {
    let playing = match *window.mode {
        Mode::Preview => window.machine.preview.is_playing(),
        Mode::Prepare => window.view.section.playing,
    };
    let (glyph, tooltip) = if playing {
        (icon::PAUSE, "Pause".to_owned())
    } else {
        (icon::PLAY, shortcuts::tooltip(Action::Play))
    };
    if icon_button(ui, glyph, &tooltip).clicked() {
        play(window);
    }
}

/// Where the cut stands on the strip, and what it counts in: layers of the stack in the
/// Preview mode, layer heights of the model in the Prepare mode.
struct Reading {
    /// Share of the way up, from zero at the plate to one at the top, or `None` with
    /// nothing to scrub.
    at: Option<f32>,
    layer: usize,
    layers: usize,
    height_mm: Scalar,
    top_mm: Scalar,
}

impl Reading {
    fn of(window: &Window) -> Self {
        match *window.mode {
            Mode::Preview => {
                let preview = &window.machine.preview;
                let count = preview.layer_count();
                let last = count.saturating_sub(1);
                Self {
                    at: (count > 0)
                        .then(|| fraction(0.0, last as Scalar, preview.layer() as Scalar)),
                    layer: if count > 0 { preview.layer() + 1 } else { 0 },
                    layers: count,
                    height_mm: preview.layer_z().unwrap_or_default(),
                    top_mm: preview.stack_top_mm().unwrap_or_default(),
                }
            }
            Mode::Prepare => {
                let Some((bottom, top)) = height_range(window.doc) else {
                    return Self::nothing();
                };
                let height = window.view.section.height_mm.unwrap_or(top);
                let (layer, layers) = layer_at(
                    height,
                    (bottom, top),
                    window.machine.slicing.layer_height_mm(),
                );
                Self {
                    at: Some(fraction(bottom, top, height)),
                    layer,
                    layers,
                    height_mm: height,
                    top_mm: top,
                }
            }
        }
    }

    fn nothing() -> Self {
        Self {
            at: None,
            layer: 0,
            layers: 0,
            height_mm: 0.0,
            top_mm: 0.0,
        }
    }

    /// The layer the cut stands at, over how many there are and its height, centred.
    /// A click on the layer opens it for typing; answers with the layer typed, on Enter.
    fn show(&self, ui: &mut egui::Ui) -> Option<usize> {
        let colors = theme::colors();
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(vec2(width, READOUT_H), Sense::hover());
        if self.at.is_none() {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "No layers",
                theme::small(),
                colors.text_low,
            );
            response.on_hover_text("No layers yet");
            return None;
        }
        let painter = ui.painter();
        let row_h = painter
            .layout_no_wrap("0".to_owned(), theme::figures(12.5), colors.text_low)
            .size()
            .y;
        painter.text(
            rect.center_top() + vec2(0.0, row_h),
            Align2::CENTER_TOP,
            format!("/ {}", self.layers),
            theme::figures(11.0),
            colors.text_low,
        );
        painter.text(
            rect.center_bottom(),
            Align2::CENTER_BOTTOM,
            mm(self.height_mm),
            theme::figures(10.0),
            colors.text_low,
        );
        response.on_hover_text(format!("{:.2} of {:.2} mm", self.height_mm, self.top_mm));
        let layer_rect = Rect::from_min_size(rect.min, vec2(width, row_h));
        self.layer_field(ui, layer_rect)
    }

    /// The layer's own figure, or the box it is typed into once clicked.
    fn layer_field(&self, ui: &mut egui::Ui, rect: Rect) -> Option<usize> {
        let colors = theme::colors();
        let id = egui::Id::new("layer-readout");
        let typing: Option<String> = ui.data(|data| data.get_temp(id));
        let Some(mut text) = typing else {
            let figure = ui.painter().layout_no_wrap(
                self.layer.to_string(),
                theme::figures(12.5),
                colors.accent_soft,
            );
            let at = pos2(rect.center().x - figure.size().x / 2.0, rect.top());
            let spot = Rect::from_min_size(at, figure.size());
            let response = ui
                .interact(spot, id.with("figure"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::Text)
                .on_hover_text("Type a layer to go to");
            ui.painter().galley(at, figure, colors.accent_soft);
            if response.clicked() {
                ui.data_mut(|data| data.insert_temp(id, self.layer.to_string()));
                ui.memory_mut(|memory| memory.request_focus(id.with("field")));
            }
            return None;
        };
        // The box is the figure itself: no frame, no margin, as wide as what is typed and
        // centred where it stood, so opening it moves nothing on the strip.
        let typed_w = ui
            .painter()
            .layout_no_wrap(text.clone(), theme::figures(12.5), colors.accent_soft)
            .size()
            .x;
        let field = Rect::from_center_size(
            rect.center(),
            vec2(typed_w.max(LAYER_FIELD_MIN_W), rect.height()),
        );
        let response = ui.put(
            field,
            egui::TextEdit::singleline(&mut text)
                .id(id.with("field"))
                .font(theme::figures(12.5))
                .text_color(colors.accent_soft)
                .horizontal_align(Align::Center)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .desired_width(field.width()),
        );
        if !response.lost_focus() {
            ui.data_mut(|data| data.insert_temp(id, text));
            return None;
        }
        ui.data_mut(|data| data.remove::<String>(id));
        let entered = ui.input(|input| input.key_pressed(egui::Key::Enter));
        entered.then(|| text.trim().parse().ok()).flatten()
    }
}

/// A height in the strip's narrow column: two decimals, one from a hundred millimetres up.
fn mm(height_mm: Scalar) -> String {
    if height_mm < 100.0 {
        format!("{height_mm:.2} mm")
    } else {
        format!("{height_mm:.1} mm")
    }
}

/// Moves the cut to `layer`, counted from one at the plate as the readout counts it.
fn go_to_layer(window: &mut Window, layer: usize) {
    match *window.mode {
        Mode::Preview => {
            let preview = &mut window.machine.preview;
            preview.set_layer(layer.saturating_sub(1));
            preview.set_playing(false);
        }
        Mode::Prepare => {
            let Some(range) = height_range(window.doc) else {
                return;
            };
            let layer_height_mm = window.machine.slicing.layer_height_mm();
            window.view.section.height_mm = height_of_layer(layer, range, layer_height_mm);
            window.view.section.playing = false;
        }
    }
}

/// Where a cut at `layer` stands, `layer` layer heights over the bottom of the range, or
/// `None` at the top and past it, where the whole model is shown.
fn height_of_layer(
    layer: usize,
    (bottom, top): (Scalar, Scalar),
    layer_height_mm: Scalar,
) -> Option<Scalar> {
    let height = bottom + layer as Scalar * layer_height_mm;
    (height < top).then_some(height)
}

/// Which layer from the plate up a cut at `height` is, counted in layers of
/// `layer_height_mm` from the bottom of the range, and how many layers the range holds.
fn layer_at(
    height: Scalar,
    (bottom, top): (Scalar, Scalar),
    layer_height_mm: Scalar,
) -> (usize, usize) {
    if layer_height_mm <= 0.0 || top <= bottom {
        return (0, 0);
    }
    let layers = ((top - bottom) / layer_height_mm).ceil() as usize;
    let layer = ((height - bottom) / layer_height_mm).round().max(0.0) as usize;
    (layer.min(layers), layers)
}

/// The track itself, its top the top of the print: a press or a drag puts the cut under
/// the pointer.
fn track(ui: &mut egui::Ui, window: &mut Window, height: f32, reading: &Reading) {
    // An id of its own rather than the next in line: a button appearing before the track
    // mid-drag would otherwise hand the drag to another widget.
    let (rect, _) = ui.allocate_exact_size(vec2(TRACK_W, height), Sense::hover());
    let sense = if reading.at.is_some() {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, egui::Id::new("layer-track"), sense);
    let span = rect.y_range().shrink(HANDLE.y / 2.0);
    if reading.at.is_some()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        scrub(window, span_fraction(span, pointer.y));
    }
    let at = Reading::of(window).at;
    paint_track(ui, window, rect, span, at);
    if at.is_some() {
        response.on_hover_cursor(egui::CursorIcon::ResizeVertical);
    } else {
        response.on_hover_text("Nothing on the plate to cut through yet");
    }
}

/// Moves the cut to `at` of the way up, and stops whatever was playing.
fn scrub(window: &mut Window, at: f32) {
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

/// Where along `span` the pointer at `y` stands, from zero at its bottom to one at its top.
fn span_fraction(span: egui::Rangef, y: f32) -> f32 {
    if span.span() <= 0.0 {
        return 1.0;
    }
    ((span.max - y) / span.span()).clamp(0.0, 1.0)
}

/// How far down `span` a share `at` of the way up stands.
fn up(span: egui::Rangef, at: f32) -> f32 {
    egui::lerp(span.max..=span.min, at)
}

fn paint_track(ui: &egui::Ui, window: &Window, rect: Rect, span: egui::Rangef, at: Option<f32>) {
    let colors = theme::colors();
    let painter = ui.painter();
    let rule = Rect::from_x_y_ranges(
        egui::Rangef::new(
            rect.center().x - RULE_W / 2.0,
            rect.center().x + RULE_W / 2.0,
        ),
        span.expand(HANDLE.y / 2.0),
    );

    if let Some((measured, along)) = marked(window, span) {
        Marks::of(measured, along.span().max(1.0) as usize).paint(painter, along, rule);
    }
    exposure_bands(painter, window, span, rect.left());

    painter.rect(
        rule,
        egui::CornerRadius::same(255),
        colors.sunken,
        Stroke::new(1.0, colors.hairline),
        egui::StrokeKind::Outside,
    );
    // With nothing to cut through the rule stands alone: a scale would measure nothing.
    let Some(at) = at else {
        return;
    };
    for tick in 0..TICKS {
        let y = egui::lerp(span, tick as f32 / (TICKS - 1) as f32);
        painter.hline(
            egui::Rangef::new(rect.right() - 6.0, rect.right()),
            y,
            Stroke::new(1.0, colors.line),
        );
    }
    let y = up(span, at);
    let filled = rule.with_min_y(y);
    painter.rect_filled(
        filled,
        egui::CornerRadius::same(255),
        colors.accent.gamma_multiply(0.75),
    );
    let handle = Rect::from_center_size(pos2(rect.center().x, y), HANDLE);
    painter.rect(
        handle,
        egui::CornerRadius::same(3),
        colors.text_high,
        Stroke::new(2.0, colors.accent.gamma_multiply(0.6)),
        egui::StrokeKind::Outside,
    );
}

/// The stack the marks are read from, and the stretch of the track it covers: all of it
/// in the Preview mode, and in the Prepare mode the stack's own height within the model's,
/// while the stack is still the plate's.
fn marked<'a>(
    window: &'a Window,
    span: egui::Rangef,
) -> Option<(&'a core_analysis::Measured, egui::Rangef)> {
    let measured = window.measured()?;
    if *window.mode == Mode::Preview {
        return Some((measured, span));
    }
    let preview = &window.machine.preview;
    let fingerprint = stack_fingerprint(&window.doc.scene, window.machine.slicing.cutting());
    if preview.read_facts().is_some() || preview.is_stale(fingerprint) {
        return None;
    }
    let (bottom, top) = height_range(window.doc)?;
    let stack_top = preview.stack_top_mm()?;
    let from = up(span, fraction(bottom, top, 0.0));
    let to = up(span, fraction(bottom, top, stack_top));
    Some((measured, egui::Rangef::new(to, from)))
}

/// The exposure bands down the left of the track, each in its own tint, over the height
/// the strip spans. A file being read states its own exposures, so it shows none of ours.
fn exposure_bands(painter: &egui::Painter, window: &Window, span: egui::Rangef, left: f32) {
    let (bottom_mm, top_mm) = match *window.mode {
        Mode::Preview if window.machine.preview.read_facts().is_none() => {
            match window.machine.preview.stack_top_mm() {
                Some(top) => (0.0, top),
                None => return,
            }
        }
        Mode::Prepare => match height_range(window.doc) {
            Some(range) => range,
            None => return,
        },
        Mode::Preview => return,
    };
    for (index, band) in window.machine.slicing.exposure.iter().enumerate() {
        let from = fraction(bottom_mm, top_mm, band.from_mm);
        let to = fraction(bottom_mm, top_mm, band.to_mm);
        if to <= from {
            continue;
        }
        let strip = Rect::from_x_y_ranges(left..=left + BAND_W, up(span, to)..=up(span, from));
        painter.rect_filled(
            strip,
            egui::CornerRadius::same(255),
            theme::band_tint(index),
        );
    }
}

/// What the strip shows of the stack along its track: the cured area up the print, as a
/// profile either side of the rule, and a tick at every layer named as a risk.
struct Marks {
    /// The widest layer in each row of the track, from the plate up, over the widest of
    /// all.
    widths: Vec<f32>,
    /// Where each risk sits up the stack, from zero to one, and its colour.
    risks: Vec<(f32, egui::Color32)>,
}

impl Marks {
    fn of(measured: &core_analysis::Measured, rows: usize) -> Self {
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
        let risks = measured
            .risks()
            .iter()
            .map(|risk| (risk.layer as f32 / last, risk_tint(risk)))
            .collect();
        Self { widths, risks }
    }

    fn paint(&self, painter: &egui::Painter, span: egui::Rangef, rule: Rect) {
        let colors = theme::colors();
        let rows = self.widths.len() as f32;
        let profile = self
            .widths
            .iter()
            .enumerate()
            .filter(|(_, width)| **width > 0.0)
            .map(|(row, width)| {
                let y = up(span, (row as f32 + 0.5) / rows);
                let half = RULE_W / 2.0 + width * PROFILE_W;
                let bar = Rect::from_center_size(
                    pos2(rule.center().x, y),
                    vec2(2.0 * half, (span.span() / rows).max(1.0)),
                );
                egui::Shape::rect_filled(bar, 0.0, colors.hairline)
            })
            .collect();
        painter.add(egui::Shape::Vec(profile));

        for (at, tint) in &self.risks {
            let y = up(span, *at);
            let tick = Rect::from_center_size(pos2(rule.left() - 5.0, y), vec2(10.0, 3.0));
            painter.rect_filled(tick, egui::CornerRadius::same(255), *tint);
        }
    }
}

/// Moves the cut by `layers`, which is a layer of the stack in the preview and a layer
/// height of the print in the prepare mode. The strip is one in both modes, so the keys
/// and the buttons that step it come here; see `docs/decisions/0061`.
pub(crate) fn step(window: &mut Window, layers: i64) {
    match *window.mode {
        Mode::Preview => {
            window.machine.preview.step(layers);
            window.machine.preview.set_playing(false);
        }
        Mode::Prepare => {
            let Some((bottom, top)) = height_range(window.doc) else {
                return;
            };
            let height = window.view.section.height_mm.unwrap_or(top);
            let moved = height + layers as Scalar * window.machine.slicing.layer_height_mm();
            window.view.section.height_mm = Some(moved.clamp(bottom, top)).filter(|z| *z < top);
            window.view.section.playing = false;
        }
    }
}

/// Starts or stops the transport: the stack in the preview, the cut up the model in the
/// prepare mode.
pub(crate) fn play(window: &mut Window) {
    match *window.mode {
        Mode::Preview => {
            let playing = window.machine.preview.is_playing();
            window.machine.preview.set_playing(!playing);
        }
        Mode::Prepare => {
            let running = window.view.section.playing;
            window.view.section.playing = !running && height_range(window.doc).is_some();
        }
    }
}

/// Where the cut goes on the next frame of play, or `None` once it has run past the top:
/// the whole model is shown again there, which is also where the play stops.
///
/// A cut parked at the top starts again from the bottom, so pressing play twice over runs
/// the model through twice rather than doing nothing the second time.
fn run_up(
    height_mm: Option<Scalar>,
    (bottom, top): (Scalar, Scalar),
    step_mm: Scalar,
) -> Option<Scalar> {
    let from = height_mm.unwrap_or(bottom - step_mm);
    let next = from + step_mm;
    (next < top).then(|| next.max(bottom))
}

/// Runs the cut up the model while the prepare mode is playing. Returns whether the
/// window has to keep repainting, the way `animate` does for the stack.
pub fn animate_section(
    ctx: &egui::Context,
    section: &mut Section,
    doc: &Doc,
    layer_height_mm: Scalar,
) -> bool {
    if !section.playing {
        return false;
    }
    let Some(range) = height_range(doc) else {
        section.playing = false;
        return false;
    };
    let step_mm = ctx.input(|input| input.stable_dt) * PLAY_LAYERS_PER_S * layer_height_mm;
    section.height_mm = run_up(section.height_mm, range, step_mm);
    section.playing = section.height_mm.is_some();
    true
}

/// The height the cut may be moved between: the bottom and the top of everything visible
/// on the plate. `None` when there is nothing there.
pub(crate) fn height_range(doc: &Doc) -> Option<(Scalar, Scalar)> {
    let bounds = doc.scene.world_bounds()?;
    let (bottom, top) = (bounds.mins.z, bounds.maxs.z);
    (top > bottom).then_some((bottom, top))
}

/// Advances the stack while the play button is down. Returns whether the window has to
/// keep repainting.
pub fn animate(ctx: &egui::Context, preview: &mut Preview) -> bool {
    if !preview.is_playing() || preview.layer_count() == 0 {
        return false;
    }
    let step = (ctx.input(|input| input.stable_dt) * PLAY_LAYERS_PER_S).round() as i64;
    preview.step(step.max(1));
    if preview.layer() + 1 == preview.layer_count() {
        preview.set_playing(false);
    }
    true
}

/// Where a height sits in its range, from zero at the bottom to one at the top.
fn fraction(bottom: Scalar, top: Scalar, height: Scalar) -> f32 {
    if top <= bottom {
        return 1.0;
    }
    ((height - bottom) / (top - bottom)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strip_marks_the_widest_layer_full_height_and_an_island_where_it_starts() {
        use core_analysis::{Measured, cure};
        use core_raster::{LayerRuns, PixelPitch};

        // Four layers on a 10 x 10 panel of 1 mm pixels: a pixel, a row, a pixel, and a
        // pixel standing on nothing.
        let layer = |lit: &[u32]| {
            let mut builder = LayerRuns::builder(10, 10);
            for &at in lit {
                builder.pad_to(at);
                builder.push(1, 255);
            }
            cure(&builder.finish(), PixelPitch { x: 1.0, y: 1.0 })
        };
        let mut measured = Measured::new(0);
        for lit in [&[0][..], &[0, 1, 2, 3], &[0], &[55]] {
            measured.push(layer(lit), 0.05);
        }

        let marks = Marks::of(&measured, 4);
        assert_eq!(marks.widths, vec![0.25, 1.0, 0.25, 0.25]);
        assert_eq!(marks.risks.len(), 1);
        assert!((marks.risks[0].0 - 1.0).abs() < 1e-6, "the top layer");
    }

    #[test]
    fn the_prepare_mode_cuts_where_the_strip_was_left() {
        let section = Section {
            height_mm: Some(3.5),
            playing: false,
        };
        let preview = Preview::default();
        assert_eq!(
            cut_height(Mode::Prepare, &section, &preview),
            Some(3.5),
            "the prepare mode carries its own height"
        );
    }

    #[test]
    fn the_preview_mode_cuts_at_the_layer_it_is_showing() {
        // An empty preview is showing no layer, so it asks for no cut whatever the
        // prepare mode was left at.
        let section = Section {
            height_mm: Some(3.5),
            playing: false,
        };
        assert_eq!(
            cut_height(Mode::Preview, &section, &Preview::default()),
            None
        );
    }

    #[test]
    fn a_cut_counts_the_layers_of_the_model_under_it() {
        // Ten millimetres at half a millimetre a layer is twenty layers.
        assert_eq!(
            layer_at(10.0, (0.0, 10.0), 0.5),
            (20, 20),
            "the top is the last"
        );
        assert_eq!(
            layer_at(2.6, (0.0, 10.0), 0.5),
            (5, 20),
            "nearest layer, not past it"
        );
        assert_eq!(
            layer_at(-1.0, (0.0, 10.0), 0.5),
            (0, 20),
            "under the plate is none"
        );
        assert_eq!(
            layer_at(5.0, (5.0, 5.0), 0.5),
            (0, 0),
            "a flat model has no layers"
        );
    }

    #[test]
    fn a_typed_layer_stands_that_many_layers_up_and_the_top_shows_everything() {
        assert_eq!(height_of_layer(4, (1.0, 10.0), 0.5), Some(3.0));
        assert_eq!(
            height_of_layer(0, (1.0, 10.0), 0.5),
            Some(1.0),
            "layer zero is the bottom"
        );
        assert_eq!(
            height_of_layer(18, (1.0, 10.0), 0.5),
            None,
            "the top cuts nothing away"
        );
        assert_eq!(
            height_of_layer(900, (1.0, 10.0), 0.5),
            None,
            "past the top is the top"
        );
    }

    #[test]
    fn the_track_reads_the_pointer_from_its_bottom_end_and_clamps_past_either() {
        let span = egui::Rangef::new(100.0, 300.0);
        assert_eq!(span_fraction(span, 200.0), 0.5);
        assert_eq!(span_fraction(span, 300.0), 0.0, "the bottom is the plate");
        assert_eq!(span_fraction(span, 0.0), 1.0, "over the top is the top");
        assert_eq!(span_fraction(span, 900.0), 0.0);
        assert_eq!(
            span_fraction(egui::Rangef::new(5.0, 5.0), 5.0),
            1.0,
            "a track with no length stands at the top"
        );
    }

    #[test]
    fn the_cut_runs_up_the_model_and_stops_at_the_top() {
        let range = (0.0, 10.0);
        assert_eq!(
            run_up(None, range, 1.0),
            Some(0.0),
            "a cut parked at the top starts again from the bottom"
        );
        assert_eq!(run_up(Some(4.0), range, 1.0), Some(5.0));
        assert_eq!(
            run_up(Some(9.5), range, 1.0),
            None,
            "past the top the whole model is shown again, and the play stops"
        );
    }

    #[test]
    fn a_range_of_no_height_reads_as_full() {
        assert_eq!(fraction(5.0, 5.0, 5.0), 1.0);
        assert_eq!(fraction(0.0, 10.0, 2.5), 0.25);
        assert_eq!(
            fraction(0.0, 10.0, 40.0),
            1.0,
            "a value past the top clamps"
        );
    }
}

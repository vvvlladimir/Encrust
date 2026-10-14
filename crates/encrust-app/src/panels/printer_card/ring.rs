//! The ring at the left of the print chip: how far the current printer has got, in the
//! colour of what it is doing. A press opens that printer, or a list of every printer on
//! the network when there is more than one. See `docs/decisions/0264`.

use egui::{Align2, Pos2, RichText, Sense, Shape, Stroke, pos2, vec2};

use super::{Light, colour_of, face, left};
use crate::network::{Network, Reading, Target};
use crate::panels::Window;
use crate::ui::{focus_ring, icon, theme};

/// The ring's box, as tall as the chip beside it, and how thick its track is drawn.
const SIZE: f32 = theme::CHIP_H;
const TRACK_W: f32 = 3.0;
/// The ring in a row of the list.
const SMALL: f32 = 22.0;
/// Points between the ring and the chip.
pub const GAP: f32 = 6.0;
/// How wide a row of the list is drawn.
const ROW_W: f32 = 300.0;
const ROW_H: f32 = 46.0;
/// Share of a turn the arc spans while a print runs without a count yet.
const SPINNER_SHARE: f32 = 0.25;
/// Turns a second the spinner makes.
const SPINNER_TURNS_S: f32 = 0.8;

/// Draws the ring when the window knows any printer, and answers the width it took.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) -> f32 {
    if cfg!(target_arch = "wasm32") {
        return 0.0;
    }
    let network = &mut window.machine.network;
    network.keep_fresh(ui.input(|input| input.time));
    let Some(current) = network.current() else {
        return 0.0;
    };
    let key = current.key();
    let count = network.targets().len();
    let look = Look::of(network, &current);
    let hover = hover_line(network, &current);
    let (rect, response) = ui.allocate_exact_size(vec2(SIZE, SIZE), Sense::click());
    let response = response.on_hover_text(hover);
    let hovered = response.hovered();
    paint(ui, rect.center(), SIZE / 2.0 - 2.0, &look, hovered);
    if count > 1 {
        badge(ui, rect, count);
    }
    focus_ring(ui, &response, theme::R_CONTROL);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Printers on the network")
    });

    if count == 1 {
        if response.clicked() {
            network.open = Some(key);
        }
    } else {
        let mut opened = None;
        egui::Popup::menu(&response).show(|ui| opened = list(ui, network));
        if opened.is_some() {
            network.open = opened;
        }
    }
    SIZE + GAP
}

/// What the ring shows for one printer.
#[derive(Debug, Clone, PartialEq)]
struct Look {
    light: Light,
    /// Share done, while there is a count to show; a print running without one spins.
    progress: Option<f32>,
    spinning: bool,
}

impl Look {
    fn of(network: &Network, target: &Target<'_>) -> Self {
        let reading = network.reading(target);
        let face = face(network.reach(target), reading);
        let (progress, busy) = match reading {
            Some(Reading::State(state)) => (state.progress.filter(|_| state.busy), state.busy),
            _ => (None, false),
        };
        let counted = progress.is_some_and(|share| share > 0.0);
        Self {
            light: face.light,
            progress: progress.filter(|_| counted),
            spinning: busy && !counted && face.light == Light::Working,
        }
    }
}

/// The track, the arc over it, and in the middle the share done or the printer glyph.
fn paint(ui: &egui::Ui, center: Pos2, radius: f32, look: &Look, hovered: bool) {
    let colors = theme::colors();
    let painter = ui.painter();
    if hovered {
        painter.circle_filled(center, radius + 2.0, colors.hover);
    }
    let colour = colour_of(look.light);
    let track = match look.light {
        Light::Ready => colour.gamma_multiply(0.45),
        Light::Working | Light::Held | Light::Down | Light::Unknown => colors.line,
    };
    painter.circle_stroke(center, radius - TRACK_W / 2.0, Stroke::new(TRACK_W, track));
    let arc = if look.spinning {
        let turn = (ui.input(|input| input.time) as f32 * SPINNER_TURNS_S).fract();
        ui.ctx().request_repaint();
        Some((turn, SPINNER_SHARE))
    } else {
        look.progress.map(|share| (0.0, share))
    };
    if let Some((from, share)) = arc {
        let points = arc_points(center, radius - TRACK_W / 2.0, from, share);
        painter.add(Shape::line(points, Stroke::new(TRACK_W, colour)));
    }
    match look.progress {
        Some(share) if radius > SMALL / 2.0 => {
            painter.text(
                center,
                Align2::CENTER_CENTER,
                format!("{:.0}", share * 100.0),
                theme::figures(10.0),
                colors.text_high,
            );
        }
        _ => {
            painter.text(
                center,
                Align2::CENTER_CENTER,
                icon::PRINTER,
                theme::icon(radius * 0.85),
                if look.light == Light::Down {
                    colors.text_low
                } else {
                    colors.text_mid
                },
            );
        }
    }
}

/// How many printers the list holds, in a small disc at the ring's corner.
fn badge(ui: &egui::Ui, rect: egui::Rect, count: usize) {
    let colors = theme::colors();
    let at = rect.right_top() + vec2(-4.0, 5.0);
    ui.painter().circle_filled(at, 6.5, colors.raised);
    ui.painter()
        .circle_stroke(at, 6.5, Stroke::new(1.0, colors.line));
    ui.painter().text(
        at,
        Align2::CENTER_CENTER,
        count.to_string(),
        theme::figures(9.0),
        colors.text_high,
    );
}

/// Points along a circle from `from` of a turn, clockwise from the top, for `share` of
/// a turn; enough of them that the arc reads as round at the ring's size.
fn arc_points(center: Pos2, radius: f32, from: f32, share: f32) -> Vec<Pos2> {
    let share = share.clamp(0.0, 1.0);
    let steps = ((share * 64.0).ceil() as usize).max(1);
    (0..=steps)
        .map(|step| {
            let turn = from + share * step as f32 / steps as f32;
            let angle = turn * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            pos2(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        })
        .collect()
}

/// The line the ring says on hover: the printer and what it is doing.
fn hover_line(network: &Network, target: &Target<'_>) -> String {
    let state = super::summary(network.reach(target), network.reading(target));
    format!("{}: {state}", network.label(target))
}

/// Every printer on the network, a row each, and the key of the one pressed.
fn list(ui: &mut egui::Ui, network: &Network) -> Option<String> {
    let mut opened = None;
    ui.set_min_width(ROW_W);
    ui.spacing_mut().item_spacing.y = 2.0;
    for target in network.targets() {
        if row(ui, network, &target).clicked() {
            opened = Some(target.key());
        }
    }
    opened
}

/// One printer in the list: its small ring, its name over what it is doing, and the time
/// left at the right.
fn row(ui: &mut egui::Ui, network: &Network, target: &Target<'_>) -> egui::Response {
    let colors = theme::colors();
    let (rect, response) = ui.allocate_exact_size(vec2(ROW_W, ROW_H), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, theme::R_CONTROL, colors.hover);
    }
    let look = Look::of(network, target);
    let ring_at = pos2(rect.left() + 8.0 + SMALL / 2.0, rect.center().y);
    paint(ui, ring_at, SMALL / 2.0, &look, false);
    let reading = network.reading(target);
    let face = face(network.reach(target), reading);
    let text_x = rect.left() + 16.0 + SMALL;
    let painter = ui.painter();
    painter.text(
        pos2(text_x, rect.center().y - 8.0),
        Align2::LEFT_CENTER,
        network.label(target),
        theme::label(),
        colors.text_high,
    );
    let line = match &face.job {
        Some(job) if job.progress.is_some() => format!("{} · {}", face.word, job.file),
        _ => face.word.clone(),
    };
    painter.text(
        pos2(text_x, rect.center().y + 9.0),
        Align2::LEFT_CENTER,
        line,
        theme::small(),
        colour_of(face.light),
    );
    let remaining = match reading {
        Some(Reading::State(state)) if state.busy => state.remaining_s.map(left),
        _ => None,
    };
    if let Some(remaining) = remaining {
        painter.text(
            pos2(rect.right() - 8.0, rect.center().y),
            Align2::RIGHT_CENTER,
            remaining,
            theme::small(),
            colors.text_mid,
        );
    }
    response.on_hover_text(RichText::new("Open this printer").font(theme::small()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use printer_link::State;

    fn one_board() -> Network {
        let board = net_sdcp::Printer {
            name: "3D Printer".into(),
            model: "Saturn 4 Ultra".into(),
            brand: "ELEGOO".into(),
            address: std::net::IpAddr::from([192, 168, 1, 133]),
            mainboard_id: "aa".into(),
            brand_id: "00".into(),
            firmware: String::new(),
            protocol: String::new(),
            transport: net_sdcp::Transport::WebSocket,
        };
        let mut network = Network::default();
        network.printers = vec![board];
        network
    }

    fn look_while(state: State) -> Look {
        let mut network = one_board();
        network.hear("sdcp:aa", Reading::State(state));
        let target = network.current().expect("one board is listed");
        Look::of(&network, &target)
    }

    #[test]
    fn a_print_with_a_count_fills_the_ring_and_one_without_spins() {
        let running = State {
            state: "printing".into(),
            busy: true,
            stage: Some("exposing".into()),
            progress: Some(0.42),
            ..State::default()
        };
        let counted = look_while(running.clone());
        assert_eq!(counted.progress, Some(0.42));
        assert!(!counted.spinning);
        let homing = State {
            stage: Some("homing".into()),
            progress: Some(0.0),
            ..running
        };
        let starting = look_while(homing);
        assert_eq!(
            starting.progress, None,
            "layer 0 of 200 is no share to draw"
        );
        assert!(starting.spinning);
    }

    #[test]
    fn a_ready_printer_draws_no_arc_and_one_not_heard_from_none_either() {
        let ready = look_while(State {
            state: "idle".into(),
            progress: Some(1.0),
            ..State::default()
        });
        assert_eq!(ready.light, Light::Ready);
        assert_eq!(
            ready.progress, None,
            "a finished print leaves no share behind"
        );
        let network = one_board();
        let target = network.current().expect("one board is listed");
        let silent = Look::of(&network, &target);
        assert!(silent.progress.is_none() && !silent.spinning);
    }

    #[test]
    fn an_arc_starts_at_the_top_and_runs_clockwise_for_its_share() {
        let center = pos2(0.0, 0.0);
        let quarter = arc_points(center, 10.0, 0.0, 0.25);
        let first = quarter.first().expect("an arc has points");
        let last = quarter.last().expect("an arc has points");
        assert!(
            (first.x).abs() < 1e-4 && (first.y + 10.0).abs() < 1e-4,
            "top: {first:?}"
        );
        assert!(
            (last.x - 10.0).abs() < 1e-4 && last.y.abs() < 1e-4,
            "right: {last:?}"
        );
    }

    #[test]
    fn a_share_past_the_whole_is_drawn_as_the_whole() {
        let center = pos2(0.0, 0.0);
        let whole = arc_points(center, 10.0, 0.0, 1.5);
        let (first, last) = (whole[0], whole[whole.len() - 1]);
        assert!(
            (first - last).length() < 1e-3,
            "a full turn closes on itself"
        );
    }
}

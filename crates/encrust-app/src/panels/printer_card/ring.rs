//! The ring left of the print chip: the current printer's progress in the colour of its state.
//! A press opens that printer or the list of printers. See `docs/decisions/0264`.

use egui::{Align2, Pos2, RichText, Sense, pos2, vec2};

use super::{Light, colour_of, face, left};
use crate::i18n::tr;
use crate::network::{Network, Reading, Target};
use crate::panels::Window;
pub use crate::ui::theme::layout::printer_card_ring::GAP;
use crate::ui::theme::layout::printer_card_ring::{ROW_H, ROW_W, SIZE, SMALL};
use crate::ui::{RingLook, focus_ring, icon, paint_ring, ring_badge, theme};

/// Draws the ring when the window knows any printer, and answers the width it took.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) -> f32 {
    if cfg!(target_arch = "wasm32") {
        return 0.0;
    }
    let network = &mut window.machine.network;
    network.keep_fresh(ui.input(|input| input.time));
    let printing = network
        .in_view()
        .iter()
        .any(|target| matches!(network.reading(target), Some(Reading::State(state)) if state.busy));
    let Some(current) = network.current().filter(|_| printing) else {
        return 0.0;
    };
    let key = current.key();
    let count = network.in_view().len();
    let look = Look::of(network, &current);
    let hover = hover_line(network, &current);
    let (rect, response) = ui.allocate_exact_size(vec2(SIZE, SIZE), Sense::click());
    let response = response.on_hover_text(hover);
    let hovered = response.hovered();
    paint(ui, rect.center(), SIZE / 2.0 - 2.0, &look, hovered);
    if count > 1 {
        ring_badge(ui, rect, count);
    }
    focus_ring(ui, &response, theme::R_CONTROL);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tr!("ring-printers"))
    });

    if count == 1 {
        if response.clicked() {
            network.open = Some(key);
        }
    } else {
        let mut opened = None;
        crate::ui::popup_menu(&response).show(|ui| opened = list(ui, network));
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
    let colour = colour_of(look.light);
    let ring = RingLook {
        glyph: icon::PRINTER,
        glyph_tint: if look.light == Light::Down {
            colors.text_low
        } else {
            colors.text_mid
        },
        track: match look.light {
            Light::Ready => colour.gamma_multiply(0.45),
            Light::Working | Light::Held | Light::Down | Light::Unknown => colors.line,
        },
        arc: colour,
        share: look.progress,
        spinning: look.spinning,
    };
    paint_ring(ui, center, radius, &ring, hovered);
}

/// The line the ring says on hover: the printer and what it is doing.
fn hover_line(network: &Network, target: &Target<'_>) -> String {
    let (state, _) = super::summary(network.reach(target), network.reading(target));
    format!("{}: {state}", network.label(target))
}

/// Every printer answering on the network, a row each, and the key of the one pressed.
fn list(ui: &mut egui::Ui, network: &Network) -> Option<String> {
    let mut opened = None;
    ui.set_min_width(ROW_W);
    ui.spacing_mut().item_spacing.y = theme::ICON_GAP;
    for target in network.in_view() {
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
    response.on_hover_text(RichText::new(tr!("ring-open")).font(theme::small()))
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
        let silent = Look::of(&network, &Target::Sdcp(&network.printers[0]));
        assert!(silent.progress.is_none() && !silent.spinning);
    }
}

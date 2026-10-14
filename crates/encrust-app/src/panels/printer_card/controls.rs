use egui::RichText;
use printer_link::{Command, State};

use super::paused;
use crate::network::{Errand, Extras};
use crate::ui::{hint, icon, quiet_button, theme, tone};

/// Share of its rated lifts past which a release film is called worn.
const WORN_SHARE: f64 = 0.9;

/// Pause or Resume and Stop while a print is under way; Stop asks once more, because a
/// stopped print cannot be resumed. What is running instead, while it runs.
pub(super) fn buttons(
    ui: &mut egui::Ui,
    key: &str,
    state: &State,
    extras: Option<&Extras>,
) -> Option<Errand> {
    if !state.busy {
        return None;
    }
    if let Some(doing) = extras.and_then(|extras| extras.doing) {
        ui.horizontal(|ui| {
            ui.spinner();
            tone(ui, doing, theme::colors().text_mid);
        });
        return None;
    }
    let armed_id = egui::Id::new(("printer-stop-armed", key));
    let armed = ui.data(|data| data.get_temp::<bool>(armed_id).unwrap_or(false));
    let mut errand = None;
    let mut arm = armed;
    if armed {
        tone(
            ui,
            "Stop the print? It cannot be resumed.",
            theme::colors().warn,
        );
        ui.horizontal(|ui| {
            if quiet_button(ui, icon::STOP, "Stop the print", true, true).clicked() {
                errand = Some(Errand::Control(Command::Stop));
                arm = false;
            }
            if quiet_button(ui, "", "Keep printing", false, true).clicked() {
                arm = false;
            }
        });
    } else {
        ui.horizontal(|ui| {
            let (glyph, command) = if paused(state) {
                (icon::PLAY, Command::Resume)
            } else {
                (icon::PAUSE, Command::Pause)
            };
            if quiet_button(ui, glyph, command.label(), false, true).clicked() {
                errand = Some(Errand::Control(command));
            }
            arm = quiet_button(ui, icon::STOP, Command::Stop.label(), true, true).clicked();
        });
    }
    ui.data_mut(|data| data.insert_temp(armed_id, arm));
    errand
}

/// How worn the release film is, on printers that count its lifts.
pub(super) fn film(ui: &mut egui::Ui, state: &State) {
    let Some((line, worn)) = film_line(state) else {
        return;
    };
    let colors = theme::colors();
    tone(ui, &line, if worn { colors.warn } else { colors.text_low });
}

fn film_line(state: &State) -> Option<(String, bool)> {
    let (lifts, max) = (state.film_lifts?, state.film_lifts_max?);
    let worn = f64::from(lifts) >= f64::from(max) * WORN_SHARE;
    let line = format!("Release film {} of {} lifts", grouped(lifts), grouped(max));
    Some((line, worn))
}

/// A count with its thousands set apart, as a reading is written.
fn grouped(count: u32) -> String {
    let digits = count.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(digit);
    }
    out
}

/// The camera's stream, on printers that serve one: a press turns it on and the card shows
/// where it streams, to copy into a player.
pub(super) fn camera(ui: &mut egui::Ui, state: &State, extras: Option<&Extras>) -> Option<Errand> {
    if !state.camera {
        return None;
    }
    let free = extras.and_then(|extras| extras.doing).is_none();
    let mut errand = None;
    match extras.and_then(|extras| extras.camera.as_deref()) {
        Some(url) => {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(icon::CAMERA)
                        .font(theme::icon(14.0))
                        .color(theme::colors().ok),
                );
                ui.add(
                    egui::Label::new(
                        RichText::new(url)
                            .font(theme::code(11.0))
                            .color(theme::colors().text_high),
                    )
                    .truncate(),
                );
            });
            ui.horizontal(|ui| {
                if quiet_button(ui, icon::CLIPBOARD, "Copy", false, true).clicked() {
                    ui.ctx().copy_text(url.to_owned());
                }
                if quiet_button(ui, "", "Turn the camera off", false, free).clicked() {
                    errand = Some(Errand::Camera(false));
                }
            });
            hint(
                ui,
                "The stream is RTSP: open the address in a player that takes it.",
            );
        }
        None => {
            if quiet_button(ui, icon::CAMERA, "Camera stream", false, free).clicked() {
                errand = Some(Errand::Camera(true));
            }
        }
    }
    errand
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_film_is_counted_against_its_rating_and_called_worn_near_the_end() {
        let mut state = State {
            film_lifts: Some(668),
            film_lifts_max: Some(60_000),
            ..State::default()
        };
        let (line, worn) = film_line(&state).expect("the board counts its lifts");
        assert_eq!(line, "Release film 668 of 60\u{202f}000 lifts");
        assert!(!worn);
        state.film_lifts = Some(54_000);
        assert!(
            film_line(&state).is_some_and(|(_, worn)| worn),
            "90 % of 60 000"
        );
        state.film_lifts_max = None;
        assert_eq!(
            film_line(&state),
            None,
            "no rating, nothing to measure against"
        );
    }

    #[test]
    fn thousands_are_set_apart() {
        assert_eq!(grouped(7), "7");
        assert_eq!(grouped(1_000), "1\u{202f}000");
        assert_eq!(grouped(1_234_567), "1\u{202f}234\u{202f}567");
    }
}

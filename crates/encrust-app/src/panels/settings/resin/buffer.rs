use printer_profiles::{Buffer, MaterialProfile};

use super::motion::UNLIMITED;
use super::{COARSE_STEP, held_seconds_row};
use crate::i18n::tr;
use crate::ui::{Held, limited_row, switch};

/// The buffer the machine's firmware runs: whether it does, over how much travel, how fast,
/// and the rest after it. Starts from `machine`'s own until the resin says otherwise.
pub(super) fn rows(ui: &mut egui::Ui, values: &mut MaterialProfile, machine: Buffer) {
    let mut buffer = values.buffer.unwrap_or(machine);
    let before = buffer;
    switch(ui, &mut buffer.enabled, &tr!("resin-buffer-on"));
    ui.add_enabled_ui(buffer.enabled, |ui| {
        let held = || Held {
            limit: &UNLIMITED,
            why: None,
        };
        limited_row(
            ui,
            &tr!("resin-buffer-height"),
            &mut buffer.height_mm,
            &tr!("unit-mm"),
            COARSE_STEP,
            0.0..=20.0,
            Some(2),
            held(),
        );
        limited_row(
            ui,
            &tr!("resin-buffer-speed"),
            &mut buffer.speed_mm_min,
            &tr!("unit-mm-min"),
            1.0,
            1.0..=1200.0,
            Some(1),
            held(),
        );
        held_seconds_row(
            ui,
            &tr!("resin-buffer-light-off"),
            &mut buffer.light_off_delay_s,
            held(),
        );
    });
    if buffer != before {
        values.buffer = Some(buffer);
    }
}

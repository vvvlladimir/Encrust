use printer_profiles::{Buffer, MaterialProfile};

use super::{COARSE_STEP, FINE_STEP};
use crate::i18n::tr;
use crate::ui::{number_row, switch};

/// The buffer the machine's firmware runs: whether it does, over how much travel, how fast,
/// and the rest after it. Starts from `machine`'s own until the resin says otherwise.
pub(super) fn rows(ui: &mut egui::Ui, values: &mut MaterialProfile, machine: Buffer) {
    let mut buffer = values.buffer.unwrap_or(machine);
    let before = buffer;
    switch(ui, &mut buffer.enabled, &tr!("resin-buffer-on"));
    ui.add_enabled_ui(buffer.enabled, |ui| {
        number_row(
            ui,
            &tr!("resin-buffer-height"),
            &mut buffer.height_mm,
            &tr!("unit-mm"),
            COARSE_STEP,
            0.0..=20.0,
            2,
        );
        number_row(
            ui,
            &tr!("resin-buffer-speed"),
            &mut buffer.speed_mm_min,
            &tr!("unit-mm-min"),
            1.0,
            1.0..=1200.0,
            1,
        );
        number_row(
            ui,
            &tr!("resin-buffer-light-off"),
            &mut buffer.light_off_delay_s,
            &tr!("unit-s"),
            FINE_STEP,
            0.0..=60.0,
            2,
        );
    });
    if buffer != before {
        values.buffer = Some(buffer);
    }
}

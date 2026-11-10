//! A file sliced before, here or by anything else, sent as it is to the printer the plate's
//! card sends to; once it lands, the card's press starts it as it starts a sliced plate.

use std::path::PathBuf;

use core_pipeline::{CtbVersion, SlicedFormat};
use printer_profiles::PrinterProfile;

use crate::i18n::tr;
use crate::network::Target;
use crate::panels::destination;
use crate::state::Machine;
use crate::status::Status;

/// The File menu row's printer name and why it cannot run now; `None` hides the row,
/// because the printer in hand sends to no printer.
pub fn offer(machine: &Machine) -> Option<(String, Option<String>)> {
    let connection = destination::connection(machine);
    let (name, refused) = machine.network.offer_picked(connection)?;
    let slicing = machine
        .slicing
        .is_sending()
        .then(|| tr!("network-sending", name = name.clone()));
    Some((name, refused.or(slicing)))
}

/// Asks for a sliced file and sends it to the bound printer, refused when its extension
/// names no sliced file or one the printer does not print.
pub fn send_file(machine: &mut Machine) {
    if !matches!(offer(machine), Some((_, None))) {
        return;
    }
    let prusa = matches!(machine.network.target(), Some(Target::Prusa(_)));
    let Some(path) = pick(machine, prusa) else {
        return;
    };
    let refused = match SlicedFormat::of(&path, CtbVersion::V5) {
        None => Some(tr!(
            "send-file-not-sliced",
            file = crate::files::file_name(&path)
        )),
        Some(format) => machine.network.blocker(format),
    };
    match refused {
        Some(why) => machine.status = Status::Error(why),
        None => machine.network.send(path, false, &mut machine.status),
    }
}

fn pick(machine: &Machine, prusa: bool) -> Option<PathBuf> {
    let extensions = sendable(machine.slicing.printer.as_ref(), prusa);
    let mut dialog = rfd::FileDialog::new().add_filter(tr!("filter-sliced"), &extensions);
    if let Some(folder) = machine.general.sliced_dir.as_deref() {
        dialog = dialog.set_directory(folder);
    }
    dialog.pick_file()
}

/// The extensions the dialog offers: `.sl1` and `.sl1s` to a Prusa machine, whatever the
/// profile writes to a board, each once.
fn sendable(printer: Option<&PrinterProfile>, prusa: bool) -> Vec<&'static str> {
    let offered: Vec<SlicedFormat> = match (prusa, printer) {
        (false, Some(printer)) => SlicedFormat::choices_for(printer).collect(),
        (false, None) => SlicedFormat::CHOICES.to_vec(),
        (true, _) => SlicedFormat::CHOICES
            .into_iter()
            .filter(|format| matches!(format, SlicedFormat::Sl1(_)))
            .collect(),
    };
    let mut extensions: Vec<&'static str> = Vec::new();
    for extension in offered.into_iter().map(SlicedFormat::extension) {
        if !extensions.contains(&extension) {
            extensions.push(extension);
        }
    }
    extensions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prusa_machine_is_offered_its_two_containers_alone() {
        assert_eq!(sendable(None, true), ["sl1", "sl1s"]);
    }

    #[test]
    fn a_board_is_offered_each_extension_once() {
        let extensions = sendable(None, false);
        assert!(extensions.contains(&"goo") && extensions.contains(&"ctb"));
        let mut once = extensions.clone();
        once.sort_unstable();
        once.dedup();
        assert_eq!(once.len(), extensions.len(), "{extensions:?}");
    }
}

//! A profile carried in or out as its file, and one thrown away (ADR 0233).

use printer_profiles::Kind;

use super::{machines, report};
use crate::files::{self, Handed, Wanted};
use crate::i18n::tr;
use crate::panels::Window;
use crate::settings::{Node, Tab};
use crate::state::Machine;
use crate::status::Status;

/// Asks for a profile's file and takes it in. A browser answers on a later frame, through
/// [`import_profile`].
pub(in crate::panels) fn ask_for_profile(window: &mut Window, kind: Kind) {
    if let Some(file) = files::pick(Wanted::Profile(kind)) {
        import_profile(window, kind, &file);
    }
}

/// Takes `file` in as a profile of `kind` and opens it where it was asked for from: a
/// resin goes on the machine the Machine and resin window has open.
pub fn import_profile(window: &mut Window, kind: Kind, file: &Handed) {
    let machine = &mut *window.machine;
    let printer = match kind {
        Kind::Resin => machine
            .settings
            .node
            .as_ref()
            .map(|node| node.printer().to_owned()),
        _ => None,
    };
    let Some(id) = crate::exchange::import_file(machine, kind, file, printer.as_deref()) else {
        return;
    };
    let settings = &mut machine.settings;
    let catalogue = &machine.slicing.catalogue;
    match (kind, printer) {
        (Kind::Printer, _) => {
            settings.library = None;
            settings.pick(catalogue, Node::Printer(id.clone()));
            machines::fill_empty_plate(window, &id);
        }
        (Kind::Resin, Some(printer)) => {
            settings.editing_resin = false;
            settings.pick(catalogue, Node::Resin { printer, resin: id });
        }
        (Kind::Support, _) => {
            settings.editing_support = false;
            settings.pick_support(catalogue, &id);
        }
        (Kind::Hollow | Kind::Drain, _) => settings.preset = Some(id),
        (Kind::Resin, None) | (Kind::Auto, _) => {}
    }
    // A plate with no resin takes the first one its machine now has.
    if matches!(kind, Kind::Printer | Kind::Resin) {
        window.machine.slicing.reload_resin();
    }
}

/// Takes a file dropped on the Machine and resin window in as a profile: a resin when the
/// window shows a machine's resins or the file can be nothing else, a machine otherwise.
pub fn import_dropped(window: &mut Window, file: &Handed) {
    let settings = &window.machine.settings;
    let resins_in_view =
        settings.library.is_none() && settings.node.is_some() && settings.tab == Tab::Resins;
    let path = file.path();
    let only_a_resin =
        printer_profiles::ForeignResin::reads(path) || core_pipeline::reads_sliced_file(path);
    let kind = match resins_in_view || only_a_resin {
        true => Kind::Resin,
        false => Kind::Printer,
    };
    import_profile(window, kind, file);
}

/// Writes the profile of `kind` under `id` out to a file the user names.
pub(in crate::panels) fn export_profile(machine: &mut Machine, kind: Kind, id: &str) {
    let text = machine.slicing.catalogue.export(kind, id);
    let Some(text) = report(&mut machine.status, &tr!("settings-cannot-export"), text) else {
        return;
    };
    let name = crate::exchange::file_name(id);
    match files::save_text(&name, Wanted::Profile(kind), &text) {
        Ok(Some(path)) => {
            machine.status =
                Status::Info(tr!("status-exported", path = path.display().to_string()));
        }
        Ok(None) => {}
        Err(error) => machine.status = Status::failed(&error),
    }
}

/// Throws away a support profile or a tool's preset, and leaves nothing of it open.
pub(in crate::panels) fn forget_profile(machine: &mut Machine, kind: Kind, id: &str) {
    let catalogue = &mut machine.slicing.catalogue;
    let outcome = catalogue.forget_user_copy(kind, id);
    report(&mut machine.status, &tr!("settings-cannot-delete"), outcome);
    let settings = &mut machine.settings;
    settings.editing_support = false;
    settings.preset = None;
    if kind == Kind::Support {
        let first = catalogue.supports().next().map(|entry| entry.id.clone());
        settings.support = None;
        if let Some(first) = first {
            settings.pick_support(catalogue, &first);
        }
    }
}

use crate::i18n::tr;
use crate::state::Doc;
use crate::ui::{describe, icon, secondary_button, section};

/// The name of what is picked, or how many, for the inspector's heading.
pub fn fact(doc: &Doc) -> Option<String> {
    match doc.scene.selection() {
        [] => None,
        [id] => doc.scene.get(*id).map(|object| object.name.clone()),
        picked => Some(tr!("models-selected", count = picked.len())),
    }
}

/// Until something is picked, how to pick it and the selection that needs no aim.
pub fn nothing_picked(ui: &mut egui::Ui, doc: &mut Doc) {
    let count = doc.scene.here().count();
    section(
        ui,
        &tr!("select-selection"),
        Some(&tr!("select-on-plate", count = count)),
        |ui| {
            describe(ui, &tr!("select-how"));
            if secondary_button(ui, icon::SELECT, &tr!("action-select-all")).clicked() {
                doc.scene.select_here();
            }
        },
    );
}

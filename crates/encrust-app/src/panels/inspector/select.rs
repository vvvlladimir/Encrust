use crate::state::Doc;
use crate::ui::{describe, icon, secondary_button, section};

/// The name of what is picked, or how many, for the inspector's heading.
pub fn fact(doc: &Doc) -> Option<String> {
    match doc.scene.selection() {
        [] => None,
        [id] => doc.scene.get(*id).map(|object| object.name.clone()),
        picked => Some(format!("{} selected", picked.len())),
    }
}

/// Until something is picked, how to pick it and the selection that needs no aim.
pub fn nothing_picked(ui: &mut egui::Ui, doc: &mut Doc) {
    let count = doc.scene.here().count();
    section(
        ui,
        "Selection",
        Some(&format!("{count} on the plate")),
        |ui| {
            describe(
                ui,
                "Click a model to pick it. Cmd-click adds one to the selection and \
                 shift-click takes everything between, and every tool then works on what \
                 is picked.",
            );
            if secondary_button(ui, icon::SELECT, "Select all").clicked() {
                doc.scene.select_here();
            }
        },
    );
}

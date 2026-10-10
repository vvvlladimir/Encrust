use crate::panels::Window;
use crate::state::Doc;
use crate::ui::{describe, icon, readings, secondary_button, section};

/// What is picked, and nothing to change it with: every other tool works on it. Until
/// something is picked, the two selections that need no aim.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let picked = window.doc.scene.selection().to_vec();
    let Some(first) = picked.first().and_then(|id| window.doc.scene.get(*id)) else {
        nothing_picked(ui, window.doc);
        return;
    };

    let mut rows = Vec::new();
    if let Some(bounds) = first.world_bounds() {
        let size = bounds.maxs - bounds.mins;
        rows.extend([
            ("Width", format!("{:.2} mm", size.x)),
            ("Depth", format!("{:.2} mm", size.y)),
            ("Height", format!("{:.2} mm", size.z)),
        ]);
    }
    rows.push(("Triangles", first.mesh.faces.len().to_string()));
    let title = if picked.len() > 1 {
        "Size of the first"
    } else {
        "Size"
    };
    section(ui, title, None, |ui| readings(ui, &rows));

    // TODO(step-8): a Problems section when the selection reaches past the build volume,
    // with a press that moves it back in.
}

/// The name of what is picked, or how many, for the inspector's heading.
pub fn fact(doc: &Doc) -> Option<String> {
    match doc.scene.selection() {
        [] => None,
        [id] => doc.scene.get(*id).map(|object| object.name.clone()),
        picked => Some(format!("{} selected", picked.len())),
    }
}

fn nothing_picked(ui: &mut egui::Ui, doc: &mut Doc) {
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

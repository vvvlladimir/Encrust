//! The recipe Prepare for print takes, picked from the built-in ones and the user's, and the
//! plan it makes for the models in hand (ADR 0405).

use core_recipe::{Listed, Origin};

use crate::i18n::tr;
use crate::panels::Window;
use crate::prepare::{PlanLine, foreseen_lines, running_lines};
use crate::ui::{
    DropdownFace, DropdownRow, Lead, PlanRow, busy, dropdown, dropdown_menu, dropdown_row,
    group_name, hint, icon, plan_row, subheading,
};

/// The picker of recipes: the built-in ones, then the user's; one that cannot run says why
/// under it, and the button over the plan says it too.
pub(super) fn picker(ui: &mut egui::Ui, window: &mut Window) {
    let listed = window
        .tools
        .prepare
        .library
        .listed(&window.machine.slicing.catalogue);
    let chosen = &window.tools.prepare.recipe;
    let picked = listed.iter().find(|entry| entry.id == *chosen);
    let name = picked.map_or(chosen.as_str(), Listed::name);
    let face = DropdownFace::plain(icon::PREPARE, name);
    let running = window.tools.prepare.is_running();
    let response = ui
        .add_enabled_ui(!running, |ui| dropdown(ui, &face, ui.available_width()))
        .inner
        .on_hover_text(tr!("position-prepare-recipe"));
    let mut choice = None;
    dropdown_menu(&response, |ui| {
        for (origin, title) in [
            (Origin::BuiltIn, tr!("position-prepare-built-in")),
            (Origin::User, tr!("position-prepare-yours")),
        ] {
            let group: Vec<&Listed> = listed.iter().filter(|e| e.origin == origin).collect();
            if group.is_empty() {
                continue;
            }
            group_name(ui, &title);
            for entry in group {
                let lead = entry.plan().is_err().then_some(Lead::Glyph(icon::WARNING));
                let row = DropdownRow {
                    lead,
                    name: entry.name(),
                    aside: None,
                    chosen: entry.id == *chosen,
                };
                if dropdown_row(ui, &row).clicked() {
                    choice = Some(entry.id.clone());
                }
            }
        }
    });
    let why = match picked {
        Some(entry) => entry.plan().err().map(ToString::to_string),
        None => Some(tr!("prepare-recipe-missing", recipe = chosen.as_str())),
    };
    if let Some(why) = why {
        hint(ui, &why);
    }
    if let Some(id) = choice {
        window.tools.prepare.recipe = id;
    }
}

/// What each step of the picked recipe will do with the models in hand, or, while a run
/// goes, where each of its steps stands.
pub(super) fn plan(ui: &mut egui::Ui, window: &mut Window) {
    let lines = match window.tools.prepare.marks() {
        Some((plan, marks)) => Some(running_lines(plan, marks)),
        None => foreseen(window),
    };
    let Some(lines) = lines else {
        if window.doc.scene.target_count() > 0 {
            subheading(ui, &tr!("position-prepare-plan"));
            busy(ui, &tr!("position-prepare-measuring"), None, false);
        }
        return;
    };
    if lines.is_empty() {
        return;
    }
    subheading(ui, &tr!("position-prepare-plan"));
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for line in &lines {
            row(ui, line);
        }
    });
}

/// The plan before a run, `None` while its models are being measured or when the recipe
/// cannot run, which the picker says.
fn foreseen(window: &mut Window) -> Option<Vec<PlanLine>> {
    let catalogue = &window.machine.slicing.catalogue;
    let prepare = &window.tools.prepare;
    let plan = prepare.library.plan(catalogue, &prepare.recipe).ok()?;
    let ids: Vec<_> = window.doc.scene.targets().map(|object| object.id).collect();
    if ids.is_empty() {
        return Some(Vec::new());
    }
    let foresight = &mut window.tools.prepare.foresight;
    let seen = foresight.look(&plan, &window.doc.scene, &window.doc.plate, &ids)?;
    Some(foreseen_lines(&plan, seen))
}

fn row(ui: &mut egui::Ui, line: &PlanLine) {
    plan_row(
        ui,
        &PlanRow {
            name: &line.name,
            verdict: &line.verdict,
            mark: line.mark,
            reading: line.reading.as_deref(),
            note: line.note.as_deref(),
        },
    );
}

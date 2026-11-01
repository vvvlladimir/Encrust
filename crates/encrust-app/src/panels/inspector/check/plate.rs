//! What Check lists before any layer is read: models past the build volume or in its inset,
//! and models that meet, each a click from the camera on it (ADR 0390).

use crate::i18n::tr;
use crate::panels::Window;
use crate::plate::{Astray, astray};
use crate::scene::{ObjectId, Scene};
use crate::ui::{describe, issue_list, issue_row, section, theme};

/// One issue of the plate as it stands.
#[derive(Debug, Clone, Copy)]
enum PlateIssue {
    Astray(Astray),
    Meeting([ObjectId; 2]),
}

/// The plate's issues, found as soon as the models stand still; none without a printer,
/// as the plate is a stand-in until then.
fn issues(ctx: &egui::Context, window: &Window) -> Vec<PlateIssue> {
    let (scene, plate) = (&window.doc.scene, &window.doc.plate);
    let astray = match plate.name {
        Some(_) => astray(ctx, scene, plate),
        None => Vec::new(),
    };
    let meetings = window.doc.meeting.pairs(scene);
    astray
        .into_iter()
        .map(PlateIssue::Astray)
        .chain(meetings.into_iter().map(PlateIssue::Meeting))
        .collect()
}

/// How many issues the plate has, for the heading's count.
pub(super) fn count(ctx: &egui::Context, window: &Window) -> usize {
    issues(ctx, window).len()
}

/// The plate's issues in a section of their own, whatever the layers say; a press on a
/// row flies the camera there, again to the next place two models meet.
pub(super) fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let issues = issues(ui.ctx(), window);
    if issues.is_empty() {
        return;
    }
    let scene = &window.doc.scene;
    let picked = section(ui, &tr!("check-plate"), None, |ui| {
        describe(ui, &tr!("check-plate-help"));
        issue_list(ui, "check-plate", |ui| {
            let mut picked = None;
            for issue in &issues {
                let (tint, place, detail) = words(scene, issue);
                if issue_row(ui, tint, (&place, None), &detail, false).clicked() {
                    picked = Some(*issue);
                }
            }
            picked
        })
    })
    .flatten();
    match picked {
        Some(PlateIssue::Astray(astray)) => {
            if let Some(spot) = astray.spot {
                window.tools.check.fly(spot);
            }
        }
        Some(PlateIssue::Meeting(pair)) => {
            let doc = &mut *window.doc;
            doc.meeting.fly_to(&doc.scene, |ids| ids == pair);
        }
        None => {}
    }
}

/// A row's colour, the model it names and what is wrong with it.
fn words(scene: &Scene, issue: &PlateIssue) -> (egui::Color32, String, String) {
    let colors = theme::colors();
    let name = |id| {
        scene
            .get(id)
            .map_or_else(String::new, |object| object.name.clone())
    };
    match *issue {
        PlateIssue::Astray(Astray { id, past: true, .. }) => {
            (colors.danger, name(id), tr!("check-row-past"))
        }
        PlateIssue::Astray(Astray { id, .. }) => (colors.warn, name(id), tr!("check-row-inset")),
        PlateIssue::Meeting([one, other]) => (
            colors.danger,
            name(one),
            tr!("check-row-meets", name = name(other)),
        ),
    }
}

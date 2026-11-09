//! What Check lists before any layer is read: models off the plate or in its inset, models
//! that meet, and resin a model keeps (ADR 0392, 0393).

use super::fixes::{button_row, free};
use crate::arrange::{arrange_plate, clear_the_way};
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::inspector::position::fit_the_volume;
use crate::plate::{Astray, BuildPlate, KeptResin, astray, bounds, kept_resin};
use crate::relief::ReliefMode;
use crate::scene::{ObjectId, Scene};
use crate::ui::{describe, icon, issue_list, issue_row, section, theme};
use crate::workspace::Tool;

/// One issue of the plate as it stands.
#[derive(Debug, Clone, Copy)]
enum PlateIssue {
    Astray(Astray),
    Meeting([ObjectId; 2]),
    Kept(KeptResin),
}

/// The plate's issues, found as soon as the models stand still; none of its bounds without
/// a printer, as the plate is a stand-in until then.
fn issues(ctx: &egui::Context, window: &Window) -> Vec<PlateIssue> {
    let (scene, plate) = (&window.doc.scene, &window.doc.plate);
    let astray = match plate.name {
        Some(_) => astray(ctx, scene, plate),
        None => Vec::new(),
    };
    let meetings = window.doc.meeting.pairs(scene);
    let kept = kept_resin(ctx, scene, &window.tools.hollow);
    astray
        .into_iter()
        .map(PlateIssue::Astray)
        .chain(meetings.into_iter().map(PlateIssue::Meeting))
        .chain(kept.into_iter().map(PlateIssue::Kept))
        .collect()
}

/// How many issues the plate has and the colour of the gravest, for the heading and the
/// rail's dot; `None` while it has none.
pub(super) fn tally(ctx: &egui::Context, window: &Window) -> Option<(usize, egui::Color32)> {
    let issues = issues(ctx, window);
    let gravest = issues
        .iter()
        .map(tint)
        .max_by_key(|&colour| colour == theme::colors().danger)?;
    Some((issues.len(), gravest))
}

/// `danger` for what slices wrong, a model cut off or fused to another; `warn` for the rest.
fn tint(issue: &PlateIssue) -> egui::Color32 {
    let colors = theme::colors();
    match issue {
        PlateIssue::Astray(Astray { past: true, .. }) | PlateIssue::Meeting(_) => colors.danger,
        PlateIssue::Astray(_) | PlateIssue::Kept(_) => colors.warn,
    }
}

/// The plate's issues in a section of their own, whatever the layers say; a row's press
/// flies the camera to it, or opens Hollow or Drain on a model keeping resin.
pub(super) fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let issues = issues(ui.ctx(), window);
    if issues.is_empty() {
        return;
    }
    let picked = section(ui, &tr!("check-plate"), None, |ui| {
        describe(ui, &tr!("check-plate-help"));
        let scene = &window.doc.scene;
        let picked = issue_list(ui, "check-plate", |ui| {
            let mut picked = None;
            for issue in &issues {
                let (tint, place, detail) = words(scene, issue);
                if issue_row(ui, tint, (&place, None), &detail, false).clicked() {
                    picked = Some(*issue);
                }
            }
            picked
        });
        fixes(ui, window, &issues);
        picked
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
        Some(PlateIssue::Kept(KeptResin::Sealed(id))) => open_on(window, id, Tool::Drain),
        Some(PlateIssue::Kept(KeptResin::Solid(id))) => open_on(window, id, Tool::Hollow),
        None => {}
    }
}

/// The buttons that answer the plate's issues, those with something to answer.
fn fixes(ui: &mut egui::Ui, window: &mut Window, issues: &[PlateIssue]) {
    let enabled = free(window);
    let in_the_way = issues
        .iter()
        .any(|issue| matches!(issue, PlateIssue::Astray(_) | PlateIssue::Meeting(_)));
    if in_the_way {
        let buttons = [
            (icon::SELECT, tr!("check-move-clear")),
            (icon::ARRANGE, tr!("check-arrange")),
        ];
        match button_row(ui, enabled, &buttons) {
            Some(0) => {
                let doc = &mut *window.doc;
                let pairs = doc.meeting.pairs(&doc.scene);
                let gap_mm = window.tools.array.gap_mm;
                window.machine.status = clear_the_way(&mut doc.scene, &doc.plate, &pairs, gap_mm);
            }
            Some(_) => {
                let doc = &mut *window.doc;
                let gap_mm = window.tools.array.gap_mm;
                window.machine.status = arrange_plate(&mut doc.scene, &doc.plate, gap_mm);
            }
            None => {}
        }
    }
    let too_big = too_big(&window.doc.scene, &window.doc.plate, issues);
    if !too_big.is_empty()
        && button_row(ui, enabled, &[(icon::FIT, tr!("check-fit-volume"))]).is_some()
    {
        for id in too_big {
            fit_the_volume(&mut window.doc.scene, &window.doc.plate, id);
        }
    }
    let meeting = issues.iter().find_map(|issue| match issue {
        PlateIssue::Meeting(pair) => Some(*pair),
        _ => None,
    });
    if let Some(pair) = meeting
        && button_row(ui, enabled, &[(icon::COMBINE, tr!("check-combine"))]).is_some()
    {
        window.doc.scene.select_many(&pair);
        window.tools.relief.mode = ReliefMode::Combine;
        *window.tool = Tool::Relief;
        window.view.options.inspector = true;
    }
    let sealed: Vec<ObjectId> = issues
        .iter()
        .filter_map(|issue| match issue {
            PlateIssue::Kept(KeptResin::Sealed(id)) => Some(*id),
            _ => None,
        })
        .collect();
    if !sealed.is_empty()
        && button_row(
            ui,
            enabled,
            &[(icon::AUTO_DRAIN, tr!("check-drill-drains"))],
        )
        .is_some()
    {
        window.doc.scene.select_many(&sealed);
        *window.tool = Tool::Drain;
        window.view.options.inspector = true;
        let layer_height_mm = window.machine.slicing.layer_height_mm();
        let started = window
            .tools
            .drain
            .start_auto(&window.doc.scene, layer_height_mm);
        window
            .machine
            .status
            .report(&tr!("drain-auto-looking"), started);
    }
}

/// The models past the plate that are larger than its build volume, which no move can fix.
fn too_big(scene: &Scene, plate: &BuildPlate, issues: &[PlateIssue]) -> Vec<ObjectId> {
    let room = core_geometry::Vec3::new(plate.x_mm, plate.y_mm, plate.z_mm);
    issues
        .iter()
        .filter_map(|issue| match issue {
            PlateIssue::Astray(Astray { id, past: true, .. }) => Some(*id),
            _ => None,
        })
        .filter(|&id| {
            scene
                .get(id)
                .and_then(bounds)
                .is_some_and(|box_of| (box_of.maxs - box_of.mins).cmpgt(room).any())
        })
        .collect()
}

/// Picks the model keeping resin and opens `tool` on it, its card open.
fn open_on(window: &mut Window, id: ObjectId, tool: Tool) {
    window.doc.scene.select(Some(id));
    *window.tool = tool;
    window.view.options.inspector = true;
}

/// A row's colour, the model it names and what is wrong with it.
fn words(scene: &Scene, issue: &PlateIssue) -> (egui::Color32, String, String) {
    let name = |id| {
        scene
            .get(id)
            .map_or_else(String::new, |object| object.name.clone())
    };
    let (place, detail) = match *issue {
        PlateIssue::Astray(Astray { id, past: true, .. }) => (name(id), tr!("check-row-past")),
        PlateIssue::Astray(Astray { id, .. }) => (name(id), tr!("check-row-inset")),
        PlateIssue::Meeting([one, other]) => {
            (name(one), tr!("check-row-meets", name = name(other)))
        }
        PlateIssue::Kept(KeptResin::Sealed(id)) => (name(id), tr!("check-row-sealed")),
        PlateIssue::Kept(KeptResin::Solid(id)) => (name(id), tr!("check-row-solid")),
    };
    (tint(issue), place, detail)
}

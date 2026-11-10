//! What Check lists before any layer is read: models off the plate or in its inset, models
//! that meet, and resin a model keeps (ADR 0392, 0393).

use super::cards::{self, Board, Card};
use super::fixes::{Offer, offer, report};
use crate::arrange::clear_the_way;
use crate::check::{IssueKind, Stop};
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::inspector::position::fit_the_volume;
use crate::plate::{Astray, BuildPlate, KeptResin, astray, bounds, kept_resin};
use crate::relief::ReliefMode;
use crate::scene::{ObjectId, Scene};
use crate::ui::{describe, icon, issue_line, issue_lines, section, theme};
use crate::workspace::Tool;

/// One issue of the plate as it stands.
#[derive(Debug, Clone, Copy)]
pub(super) enum PlateIssue {
    Astray(Astray),
    Meeting([ObjectId; 2]),
    Kept(KeptResin),
}

/// The plate's issues, found as soon as the models stand still; none of its bounds without
/// a printer, as the plate is a stand-in until then.
pub(super) fn issues(ctx: &egui::Context, window: &Window) -> Vec<PlateIssue> {
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
pub(super) fn ui(ui: &mut egui::Ui, window: &mut Window, board: &Board) {
    let kinds = [IssueKind::InTheWay, IssueKind::KeepsResin];
    let shows = |kind| board.has(kind) || board.answer(kind).is_some();
    if !kinds.into_iter().any(shows) {
        return;
    }
    section(ui, &tr!("check-plate"), None, |ui| {
        describe(ui, &tr!("check-plate-help"));
        for kind in kinds {
            cards::draw(ui, window, board, kind, |ui, window, turn_had| {
                let picked = rows(ui, window, &board.plate, kind);
                if let Some(picked) = picked {
                    go(window, picked);
                }
                match kind {
                    IssueKind::InTheWay => in_the_way(ui, window, &board.plate, turn_had),
                    _ => keeps_resin(ui, window, &board.plate, turn_had),
                }
            });
        }
    });
}

/// The rows of `kind`. Answers the issue of the row pressed.
fn rows(
    ui: &mut egui::Ui,
    window: &Window,
    issues: &[PlateIssue],
    kind: IssueKind,
) -> Option<PlateIssue> {
    let scene = &window.doc.scene;
    let marked = window.tools.check.stop();
    let name = format!("check-plate-{kind:?}");
    issue_lines(ui, &name, |ui| {
        let mut picked = None;
        for issue in issues.iter().filter(|issue| kind_of(issue) == kind) {
            let (ink, place, detail) = words(scene, issue);
            let ink = if kind == IssueKind::InTheWay {
                ink
            } else {
                theme::colors().text_mid
            };
            let selected = marked == Some(stop_of(issue));
            if issue_line(ui, &place, (&detail, ink), selected).clicked() {
                picked = Some(*issue);
            }
        }
        picked
    })
}

/// Goes where a pressed row names, opening the tool a model keeping resin wants.
fn go(window: &mut Window, picked: PlateIssue) {
    window.tools.check.arrive(stop_of(&picked));
    match picked {
        PlateIssue::Kept(KeptResin::Sealed(id)) => open_on(window, id, Tool::Drain),
        PlateIssue::Kept(KeptResin::Solid(id)) => open_on(window, id, Tool::Hollow),
        issue => show(window, issue),
    }
}

/// The arrows' stop at `models`: its card opened and the camera sent there, staying in Check.
pub(super) fn arrive(window: &mut Window, board: &Board, models: [ObjectId; 2]) {
    let Some(issue) = board.plate.iter().find(|issue| models_of(issue) == models) else {
        return;
    };
    window.tools.check.clear();
    window.tools.check.arrive(stop_of(issue));
    window.tools.check.reveal(kind_of(issue));
    show(window, *issue);
}

/// Sends the camera to `issue`, or for a model keeping resin picks it.
fn show(window: &mut Window, issue: PlateIssue) {
    match issue {
        PlateIssue::Astray(astray) => {
            if let Some(spot) = astray.spot {
                window.tools.check.fly(spot);
            }
        }
        PlateIssue::Meeting(pair) => {
            let doc = &mut *window.doc;
            doc.meeting.fly_to(&doc.scene, |ids| ids == pair);
        }
        PlateIssue::Kept(KeptResin::Sealed(id) | KeptResin::Solid(id)) => {
            window.doc.scene.select(Some(id));
        }
    }
}

/// The stop of the row of `issue`.
pub(super) fn stop_of(issue: &PlateIssue) -> Stop {
    Stop::Plate(models_of(issue))
}

/// The models a row of the plate is about, the same one twice for a row of one.
fn models_of(issue: &PlateIssue) -> [ObjectId; 2] {
    match *issue {
        PlateIssue::Astray(Astray { id, .. }) => [id, id],
        PlateIssue::Meeting(pair) => pair,
        PlateIssue::Kept(KeptResin::Sealed(id) | KeptResin::Solid(id)) => [id, id],
    }
}

/// The card `issue` stands on.
fn kind_of(issue: &PlateIssue) -> IssueKind {
    match issue {
        PlateIssue::Astray(_) | PlateIssue::Meeting(_) => IssueKind::InTheWay,
        PlateIssue::Kept(_) => IssueKind::KeepsResin,
    }
}

/// The plate's cards, each `answered` kind left to the hand as its fix has had its turn; a
/// solid model is a word of advice, and a model too big for the plate no move can place.
pub(super) fn cards(window: &Window, issues: &[PlateIssue], answered: &[IssueKind]) -> Vec<Card> {
    let too_big = too_big(&window.doc.scene, &window.doc.plate, issues);
    let fixable = |issue: &PlateIssue| match issue {
        PlateIssue::Astray(astray) => !too_big.contains(&astray.id),
        PlateIssue::Meeting(_) | PlateIssue::Kept(KeptResin::Sealed(_)) => true,
        PlateIssue::Kept(KeptResin::Solid(_)) => false,
    };
    [IssueKind::InTheWay, IssueKind::KeepsResin]
        .into_iter()
        .filter_map(|kind| {
            let of_kind: Vec<&PlateIssue> = issues
                .iter()
                .filter(|issue| kind_of(issue) == kind)
                .collect();
            let danger = theme::colors().danger;
            Some(Card {
                kind,
                count: of_kind.len(),
                fixable: if answered.contains(&kind) {
                    0
                } else {
                    of_kind.iter().filter(|issue| fixable(issue)).count()
                },
                tint: of_kind
                    .iter()
                    .map(|issue| tint(issue))
                    .max_by_key(|colour| *colour == danger)?,
            })
        })
        .collect()
}

/// What the in-the-way card can do.
#[derive(Clone, Copy)]
enum WayFix {
    Place,
    Fit,
    Combine,
}

/// The buttons that answer models in the way, those with something to answer.
fn in_the_way(ui: &mut egui::Ui, window: &mut Window, issues: &[PlateIssue], turn_had: bool) {
    let too_big = too_big(&window.doc.scene, &window.doc.plate, issues);
    let meeting = issues.iter().find_map(|issue| match issue {
        PlateIssue::Meeting(pair) => Some(*pair),
        _ => None,
    });
    let mut fixes = vec![WayFix::Place];
    let mut offers = vec![Offer::new(
        icon::ARRANGE,
        tr!("check-place"),
        tr!("check-place-tip"),
    )];
    if !too_big.is_empty() {
        fixes.push(WayFix::Fit);
        offers.push(Offer::new(
            icon::FIT,
            tr!("check-fit-volume"),
            tr!("check-fit-volume-tip"),
        ));
    }
    if meeting.is_some() {
        fixes.push(WayFix::Combine);
        offers.push(Offer::new(
            icon::COMBINE,
            tr!("check-combine"),
            tr!("check-combine-tip"),
        ));
    }
    let Some(pressed) = offer(ui, window, turn_had, &offers) else {
        return;
    };
    let doc = &mut *window.doc;
    let gap_mm = window.tools.array.gap_mm;
    match fixes[pressed] {
        WayFix::Place => {
            let pairs = doc.meeting.pairs(&doc.scene);
            let said = clear_the_way(&mut doc.scene, &doc.plate, &pairs, gap_mm);
            report(window, IssueKind::InTheWay, said);
        }
        WayFix::Fit => {
            for id in too_big {
                fit_the_volume(&mut doc.scene, &doc.plate, id);
            }
        }
        WayFix::Combine => {
            if let Some(pair) = meeting {
                doc.scene.select_many(&pair);
                window.tools.relief.mode = ReliefMode::Combine;
                *window.tool = Tool::Relief;
                window.view.options.inspector = true;
            }
        }
    }
}

/// A search for drain holes on every sealed model, the job's progress and word on the stage.
fn keeps_resin(ui: &mut egui::Ui, window: &mut Window, issues: &[PlateIssue], turn_had: bool) {
    let sealed: Vec<ObjectId> = issues
        .iter()
        .filter_map(|issue| match issue {
            PlateIssue::Kept(KeptResin::Sealed(id)) => Some(*id),
            _ => None,
        })
        .collect();
    if sealed.is_empty() {
        return;
    }
    let offers = [Offer::new(
        icon::AUTO_DRAIN,
        tr!("check-drill-drains"),
        tr!("check-drill-drains-tip"),
    )];
    if offer(ui, window, turn_had, &offers).is_some() {
        let layer_height_mm = window.machine.slicing.layer_height_mm();
        let scene = &window.doc.scene;
        let started = window
            .tools
            .drain
            .start_auto_on(scene, &sealed, layer_height_mm);
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

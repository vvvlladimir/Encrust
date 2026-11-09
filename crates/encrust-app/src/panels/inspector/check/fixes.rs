//! The buttons that answer what the layers show, each on its kind's card.

use core_analysis::{Risk, RiskKind};
use core_plate::{OrientGoal, OrientPreset};

use crate::check::{
    IssueKind, cup_owners, drill_cups, lever_owners, only_owner, support_islands, support_levers,
};
use crate::i18n::tr;
use crate::panels::Window;
use crate::scene::ObjectId;
use crate::shortcuts;
use crate::status::Status;
use crate::ui::{FlowButton, flow_buttons, hint, icon, theme, verdict_card};
use crate::workspace::Tool;

/// One way to answer a kind: its glyph, its words and what it does.
pub(super) struct Offer {
    pub glyph: &'static str,
    pub text: String,
    pub tip: String,
}

impl Offer {
    pub(super) fn new(glyph: &'static str, text: String, tip: String) -> Self {
        Self { glyph, text, tip }
    }
}

/// `offers` as buttons, the first the one suggested. Where a fix has had its turn the first
/// has done what it could and is left out. Answers the place in `offers` of the one pressed.
pub(super) fn offer(
    ui: &mut egui::Ui,
    window: &Window,
    turn_had: bool,
    offers: &[Offer],
) -> Option<usize> {
    let skipped = usize::from(turn_had);
    let buttons: Vec<FlowButton<'_>> = offers
        .iter()
        .skip(skipped)
        .enumerate()
        .map(|(index, offer)| FlowButton {
            glyph: offer.glyph,
            text: &offer.text,
            tip: &offer.tip,
            lead: index == 0,
        })
        .collect();
    flow_buttons(ui, free(window), &buttons).map(|index| index + skipped)
}

/// Whether a fix may start: no recipe running, and no Auto-orient already going.
fn free(window: &Window) -> bool {
    crate::prepare::holding(window.tools).is_none() && !window.tools.orient.is_running()
}

/// Says what a press did and notes it as the answer to `kind`, if it changed the plate.
pub(super) fn report(window: &mut Window, kind: IssueKind, said: Status) {
    let history = &window.doc.history;
    let changed = history.has_unrecorded(&window.doc.scene);
    window
        .tools
        .check
        .answers
        .note(history, changed, kind, &said);
    window.machine.status = said;
}

/// Starts Auto-orient for `preset`, on `only` or on the whole plate.
fn turn(window: &mut Window, preset: OrientPreset, only: Option<ObjectId>) {
    window.tools.orient.settings.preset = preset;
    crate::panels::auto_orient(window, only);
}

/// A vent into each cup by its thinnest wall, or a pose that leaves none.
pub(super) fn cups(ui: &mut egui::Ui, window: &mut Window, risks: &[Risk], turn_had: bool) {
    let help = tr!("check-cups-vent-help");
    let offers = [
        Offer::new(icon::DRAIN, tr!("check-drill-cups"), help.clone()),
        Offer::new(icon::AUTO_ORIENT, tr!("check-auto-orient"), help),
    ];
    let Some(pressed) = offer(ui, window, turn_had, &offers) else {
        return;
    };
    let (Some(settings), Some(plan)) = (
        window.machine.slicing.raster_settings(),
        window.machine.preview.plan(),
    ) else {
        return;
    };
    let stack = (plan, &settings);
    if pressed == 0 {
        let plate = (&mut window.doc.scene, &mut window.tools.drain);
        let said = drill_cups(plate, stack, risks, None);
        report(window, IssueKind::Cups, said);
    } else {
        let hole_mm = window.tools.drain.size().diameter_mm;
        let owners = cup_owners(&window.doc.scene, stack, risks, hole_mm);
        let only = only_owner(owners.iter().map(|owner| owner.ok().map(|(id, _)| id)));
        turn(window, OrientPreset::Auto, only);
    }
}

/// A support under each weak neck, or a pose that leaves none.
pub(super) fn necks(ui: &mut egui::Ui, window: &mut Window, risks: &[Risk], turn_had: bool) {
    let help = tr!("check-necks-held-help");
    let offers = [
        Offer::new(icon::SUPPORTS, tr!("check-support-levers"), help.clone()),
        Offer::new(icon::AUTO_ORIENT, tr!("check-auto-orient"), help),
    ];
    let Some(pressed) = offer(ui, window, turn_had, &offers) else {
        return;
    };
    let (Some(settings), Some(plan)) = (
        window.machine.slicing.raster_settings(),
        window.machine.preview.plan(),
    ) else {
        return;
    };
    let stack = (plan, &settings);
    if pressed == 0 {
        let plate = (&mut window.doc.scene, &mut window.tools.supports);
        let said = support_levers(plate, stack, risks, None);
        report(window, IssueKind::Necks, said);
    } else {
        let sizing = &window.tools.supports.profile;
        let owners = lever_owners(&window.doc.scene, stack, risks, sizing);
        let only = only_owner(owners.iter().map(|holds| holds.first().map(|(id, _)| *id)));
        turn(window, OrientPreset::Auto, only);
    }
}

/// The pose that pulls least, and the lift settings on a printer that leaves them to the file.
pub(super) fn peel(ui: &mut egui::Ui, window: &mut Window) {
    let lift_is_ours = window
        .machine
        .slicing
        .printer
        .as_ref()
        .is_some_and(|printer| printer.locked.lift_distance_mm.is_none());
    let mut offers = vec![Offer::new(
        icon::AUTO_ORIENT,
        tr!("check-least-peel"),
        tr!("check-least-peel-tip"),
    )];
    if lift_is_ours {
        offers.push(Offer::new(
            icon::PARAMETERS,
            tr!("check-lift-settings"),
            tr!("check-lift-settings-tip"),
        ));
    }
    match offer(ui, window, false, &offers) {
        Some(0) => {
            let least_peel = OrientPreset::Goal(OrientGoal::LeastPeel);
            turn(window, least_peel, None);
        }
        Some(_) => shortcuts::pick(window, Tool::PrintSettings),
        None => {}
    }
}

/// What the islands' card can do.
#[derive(Clone, Copy)]
enum IslandFix {
    Support,
    Remove,
    RemoveNext,
    PutBack,
}

/// The ways to answer the islands: support or remove them, or once removed, support what is
/// left hanging, take the next layer of it, or put them back.
pub(super) fn islands(
    ui: &mut egui::Ui,
    window: &mut Window,
    (risks, removed): (&[Risk], usize),
    turn_had: bool,
) {
    let slicing = &window.machine.slicing;
    let removing = slicing.remove_islands;
    if removing {
        removed_block(ui, removed);
        if slicing.hanging_passes > 0 {
            hint(
                ui,
                &tr!("check-hanging-passes", count = slicing.hanging_passes),
            );
        }
    }
    let (fixes, offers) = island_offers(risks, removing);
    if let Some(pressed) = offer(ui, window, turn_had && !fixes.is_empty(), &offers) {
        match fixes[pressed] {
            IslandFix::Support => stand_on_islands(window, risks),
            IslandFix::Remove => window.machine.slicing.remove_islands = true,
            IslandFix::RemoveNext => window.machine.slicing.hanging_passes += 1,
            IslandFix::PutBack => {
                window.machine.slicing.remove_islands = false;
                window.machine.slicing.hanging_passes = 0;
            }
        }
    }
}

fn island_offers(risks: &[Risk], removing: bool) -> (Vec<IslandFix>, Vec<Offer>) {
    let mut fixes = Vec::new();
    let mut offers = Vec::new();
    let mut add = |fix, offer| {
        fixes.push(fix);
        offers.push(offer);
    };
    if !risks.is_empty() {
        let support = Offer::new(
            icon::SUPPORTS,
            tr!("check-support-islands"),
            tr!("check-support-islands-tip"),
        );
        add(IslandFix::Support, support);
    }
    if !removing {
        let remove = Offer::new(
            icon::ERASE,
            tr!("check-remove-islands"),
            tr!("check-remove-islands-note"),
        );
        add(IslandFix::Remove, remove);
        return (fixes, offers);
    }
    let hanging = risks
        .iter()
        .filter(|risk| matches!(risk.kind, RiskKind::Floating { .. }))
        .count();
    if hanging > 0 {
        let next = Offer::new(
            icon::ERASE,
            tr!("check-remove-next", count = hanging),
            tr!("check-remove-next-note"),
        );
        add(IslandFix::RemoveNext, next);
    }
    let back = Offer::new(
        icon::RESET,
        tr!("check-put-islands-back"),
        tr!("check-put-islands-back-tip"),
    );
    add(IslandFix::PutBack, back);
    (fixes, offers)
}

/// Stands a support under every island the check found, in a group of their own built to
/// the support numbers in hand.
fn stand_on_islands(window: &mut Window, risks: &[Risk]) {
    let (Some(settings), Some(plan)) = (
        window.machine.slicing.raster_settings(),
        window.machine.preview.plan(),
    ) else {
        return;
    };
    let plate = (&mut window.doc.scene, &mut window.tools.supports);
    let said = support_islands(plate, (plan, &settings), risks, None);
    report(window, IssueKind::Islands, said);
}

/// What taking the islands out came to, on a quiet card of its own.
fn removed_block(ui: &mut egui::Ui, removed: usize) {
    let colors = theme::colors();
    let title = match removed {
        0 => tr!("check-no-islands-to-take"),
        count => tr!("check-removed", count = count),
    };
    let note = tr!("check-erased-note");
    let look = (colors.text_mid, colors.sunken);
    verdict_card(ui, icon::ERASE, look, &title, &note);
}

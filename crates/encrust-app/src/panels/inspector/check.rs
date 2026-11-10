use core_analysis::{Measured, Risk, RiskKind, equivalent_disc_mm};
use egui::Color32;

use crate::check::{IssueKind, Place, Stage, Stop};
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::section::{cut_height, height_range};
use crate::preview::plate_fingerprint;
use crate::ui::verdict_card;
use crate::ui::{describe, hint, icon, readings, section, section_with_actions, theme};
use crate::workspace::Mode;

mod cards;
mod fixes;
mod foot;
mod issues;
mod plate;

use cards::{Board, LAYER_KINDS, Layers};
pub(crate) use issues::issue_name;
use issues::{kind_of, listing};

pub(super) use foot::{ui as action, wanted as wants_foot};

/// What the stack will fail on: the plate's issues, then the layers', a card to each kind
/// with its rows and what answers them, and what pulls hardest on the film (ADR 0416).
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let board = Board::read(ui.ctx(), window);
    plate::ui(ui, window, &board);
    layers(ui, window, &board);
    if !board.has(IssueKind::Peel) {
        pull(ui, window);
    }
}

/// What Check shows of the stack: the full check once it has read the plate as it is, else
/// the quick one from the background, flagged `true` as preliminary (ADR 0419).
pub(crate) fn found<'a>(window: &'a Window) -> Option<(&'a Measured, bool)> {
    if let Some(measured) = window.measured() {
        return Some((measured, false));
    }
    let settings = window.machine.slicing.raster_settings()?;
    let fold = window.machine.slicing.fold();
    let quick = window.machine.preview.quick_measured(&settings, fold)?;
    Some((quick, true))
}

/// The section of what a read of the stack found, or the read still to come.
fn layers(ui: &mut egui::Ui, window: &mut Window, board: &Board) {
    let found = board.layers.as_ref();
    let risks = found.map_or(&[][..], |layers| &layers.risks);
    let stops = stops(board, risks);
    let arrows = [
        (icon::LAYER_UP, tr!("check-previous")),
        (icon::LAYER_DOWN, tr!("check-next")),
    ];
    let arrows = arrows
        .each_ref()
        .map(|(glyph, name)| (*glyph, name.as_str()));
    let heading: &[(&str, &str)] = if stops.is_empty() { &[] } else { &arrows };
    let title = tr!("check-layers");
    let (arrow, _) = section_with_actions(ui, &title, heading, |ui| {
        describe(ui, &tr!("check-issues-help"));
        if board.has(IssueKind::Peel) {
            describe(ui, &tr!("check-peel-help"));
        }
        for kind in LAYER_KINDS {
            card(ui, window, board, kind);
        }
        match found {
            None => waiting(ui, window),
            Some(layers) if layers.preliminary => refining(ui),
            Some(_)
                if board
                    .cards
                    .iter()
                    .all(|one| !LAYER_KINDS.contains(&one.kind)) =>
            {
                verdict(ui);
            }
            Some(_) => {}
        }
    });
    if let Some(forward) = arrow.map(|index| index == 1) {
        step(window, board, (&stops, risks), forward);
    }
}

/// Every issue the arrows stop at: the plate's, then the layers'.
fn stops(board: &Board, risks: &[Risk]) -> Vec<Stop> {
    let plate = board.plate.iter().map(plate::stop_of);
    plate.chain(issues::layer_stops(risks)).collect()
}

/// Goes to the issue after the one last gone to, or before it.
fn step(window: &mut Window, board: &Board, (stops, risks): (&[Stop], &[Risk]), forward: bool) {
    let shown = window.machine.preview.layer();
    let at = window.tools.check.stop().filter(|stop| match stop {
        Stop::Layer { layer, .. } => *layer == shown,
        Stop::Plate(_) => true,
    });
    match issues::next_stop(stops, at, shown, forward) {
        Some(Stop::Plate(models)) => plate::arrive(window, board, models),
        Some(stop) => go_to(window, &issues::issues_at(risks, stop)),
        None => {}
    }
}

/// The card of one kind the layers show, its rows a click away from their layer.
fn card(ui: &mut egui::Ui, window: &mut Window, board: &Board, kind: IssueKind) {
    let empty = Layers {
        risks: Vec::new(),
        removed: 0,
        preliminary: false,
    };
    let layers = board.layers.as_ref().unwrap_or(&empty);
    let risks: Vec<Risk> = layers
        .risks
        .iter()
        .filter(|risk| kind_of(&risk.kind) == kind)
        .copied()
        .collect();
    let shown = window.machine.preview.layer();
    cards::draw(ui, window, board, kind, |ui, window, turn_had| {
        if let Some(picked) = listing(ui, shown, &layers.risks, kind) {
            go_to(window, &picked);
        }
        match kind {
            IssueKind::Islands => {
                fixes::islands(ui, window, (&risks, layers.removed), turn_had);
            }
            IssueKind::Cups => fixes::cups(ui, window, &risks, turn_had),
            IssueKind::Necks => fixes::necks(ui, window, &risks, turn_had),
            IssueKind::Peel => {
                let rows = pull_rows(window);
                readings(ui, &rows);
                fixes::peel(ui, window);
            }
            _ => hint(ui, &tr!("check-thin-no-fix")),
        }
    });
}

/// What the check has found so far, the plate's own issues with the layers', for the
/// inspector's heading, and its colour.
pub fn fact(ctx: &egui::Context, window: &Window) -> Option<(String, Color32)> {
    let colors = theme::colors();
    if window.tools.prepare.is_holding() {
        return Some((tr!("check-fact-fixing"), colors.text_low));
    }
    let on_plate = plate::tally(ctx, window);
    let issues = |count: usize, colour| Some((tr!("check-fact-issues", count = count), colour));
    let Some((measured, _)) = found(window) else {
        if let Some((count, colour)) = on_plate {
            return issues(count, colour);
        }
        return window
            .machine
            .preview
            .is_measuring()
            .then(|| (tr!("check-fact-checking"), colors.text_low));
    };
    let layers = measured.worst().map(|worst| tint(&worst));
    match (layers, on_plate) {
        (None, None) => Some((tr!("check-fact-none"), colors.text_mid)),
        (None, Some((count, colour))) => issues(count, colour),
        (Some(colour), None) => issues(measured.risks().len(), colour),
        (Some(colour), Some((count, plate))) => {
            issues(measured.risks().len() + count, gravest(colour, plate))
        }
    }
}

/// The dot the rail's Check carries: the colour of the gravest issue, the plate's found
/// without Check open, the layers' once read.
pub fn badge(ctx: &egui::Context, window: &Window) -> Option<Color32> {
    let layers = found(window)
        .and_then(|(measured, _)| measured.worst())
        .map(|worst| tint(&worst));
    let on_plate = plate::tally(ctx, window).map(|(_, colour)| colour);
    match (layers, on_plate) {
        (Some(layers), Some(plate)) => Some(gravest(layers, plate)),
        (layers, plate) => layers.or(plate),
    }
}

/// `danger` if either colour is, else the first.
fn gravest(one: Color32, other: Color32) -> Color32 {
    if other == theme::colors().danger {
        other
    } else {
        one
    }
}

/// Before there is anything to read: the stack being read, or the way to have it cut.
fn waiting(ui: &mut egui::Ui, window: &mut Window) {
    if window.machine.preview.is_measuring() || window.machine.preview.is_building() {
        ui.horizontal(|ui| {
            crate::ui::spinner(ui);
            hint(ui, &tr!("check-reading"));
        });
        return;
    }
    hint(ui, &tr!("check-reads-layers"));
}

/// What the quick check found while the full one reads every pixel.
fn refining(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        crate::ui::spinner(ui);
        hint(ui, &tr!("check-quick-found"));
    });
}

/// Which layers pull hardest on the film as the plate lifts, above the bottom block, where
/// no layer pulls too hard to have a card of its own.
fn pull(ui: &mut egui::Ui, window: &mut Window) {
    if found(window).is_none() && !window.machine.preview.is_measuring() {
        return;
    }
    let rows = pull_rows(window);
    section(ui, &tr!("check-peel"), None, |ui| {
        describe(ui, &tr!("check-peel-help"));
        readings(ui, &rows);
    });
}

/// The readings of the pull, each `measuring` until it is known: a setting edited meanwhile
/// is measured again.
fn pull_rows(window: &Window) -> Vec<(String, String)> {
    let measured = found(window).map(|(measured, _)| measured);
    let waiting = || tr!("check-measuring");
    let hardest = measured.and_then(Measured::hardest_pull);
    let widest = measured.and_then(Measured::largest_growth);
    vec![
        (
            tr!("check-hardest-pull"),
            hardest.map_or_else(waiting, |peak| {
                tr!("check-layer-lower", layer = peak.layer + 1)
            }),
        ),
        (
            tr!("check-disc"),
            hardest.map_or_else(waiting, |peak| {
                let mm = format!("{:.1}", equivalent_disc_mm(f64::from(peak.value)));
                tr!("value-mm", value = mm)
            }),
        ),
        (
            tr!("check-widest-step"),
            widest.map_or_else(waiting, |peak| {
                tr!("check-layer-lower", layer = peak.layer + 1)
            }),
        ),
        (
            tr!("check-grows-by"),
            widest.map_or_else(waiting, |peak| {
                tr!("value-mm2", value = format!("{:.0}", peak.value))
            }),
        ),
    ]
}

/// The colour an issue is drawn in: islands in the colour the mask paints them, the rest
/// as warnings, since they only might fail.
pub fn tint(risk: &Risk) -> Color32 {
    let colors = theme::colors();
    match risk.kind {
        RiskKind::Island { .. } | RiskKind::Floating { .. } => colors.danger,
        RiskKind::Lever { .. }
        | RiskKind::Peel { .. }
        | RiskKind::SuctionCup { .. }
        | RiskKind::Thin { .. } => colors.warn,
    }
}

/// The card that says the check found nothing to fail on.
fn verdict(ui: &mut egui::Ui) {
    let colors = theme::colors();
    let title = tr!("check-nothing-found");
    let why = tr!("check-nothing-found-why");
    verdict_card(ui, icon::CLEAR, (colors.ok, colors.ok_wash), &title, &why);
}

/// Goes to the layer `risks` lie on: the slider there, the view cut at it, and the camera
/// flown to them ringed on the cut.
pub fn go_to(window: &mut Window, risks: &[Risk]) {
    let Some(first) = risks.first() else {
        return;
    };
    window.tools.check.clear();
    window.tools.check.reveal(kind_of(&first.kind));
    window.tools.check.arrive(Stop::Layer {
        layer: first.layer,
        kind: kind_of(&first.kind),
        hanging: matches!(first.kind, RiskKind::Floating { .. }),
    });
    let preview = &mut window.machine.preview;
    preview.set_layer(first.layer);
    preview.set_playing(false);
    let Some(layer_mm) = preview.section_z() else {
        return;
    };
    if *window.mode == Mode::Prepare
        && let Some((_, top)) = height_range(window.doc)
    {
        // As the strip has it, a cut at the top or past it is no cut.
        window.view.section.height_mm = Some(layer_mm).filter(|z| *z < top);
        window.view.section.playing = false;
    }
    let Some(settings) = window.machine.slicing.raster_settings() else {
        return;
    };
    let places: Vec<Place> = risks
        .iter()
        .map(|risk| Place::of(risk, &settings))
        .collect();
    let stage = Stage {
        plate: plate_fingerprint(&window.doc.scene),
        cut_mm: cut_height(*window.mode, &window.view.section, &window.machine.preview),
    };
    window.tools.check.go(&places, layer_mm, stage, tint(first));
}

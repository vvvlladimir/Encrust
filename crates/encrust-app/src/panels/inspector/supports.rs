use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use core_supports::{Part, SupportTree};

use super::{paint, scope, shape, support_picker};
use crate::i18n::tr;
use crate::job::SupportJob;
use crate::panels::Window;
use crate::panels::support_fields::{held_up, saved_as, sizes};
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::state::Doc;
use crate::status::Status;
use crate::supports::{Placing, Side, SupportTool};
use crate::ui::{
    BinLook, bin_button, compact_button, companion_button, describe, hint, icon, icon_toggle,
    instant_bin, number_row, primary_button, progress_bar, secondary_button, section, stats, theme,
    tone,
};
use crate::workspace::Tool;

/// The one card the supports are made in: grown, placed, painted and shaped, each a mode
/// of it under its heading. See `docs/decisions/0338`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    match mode(window) {
        Mode::Pick => {
            section(ui, &tr!("supports-picked"), None, |ui| editing(ui, window));
            section(ui, &tr!("supports-where"), None, |ui| {
                where_fields(ui, window)
            });
        }
        Mode::Place => {
            section(ui, &tr!("supports-place"), None, |ui| {
                paint::place_fields(ui, window)
            });
            section(ui, &tr!("supports-where"), None, |ui| {
                where_fields(ui, window)
            });
        }
        Mode::Area => {
            section(ui, &tr!("paint-brush"), None, |ui| {
                paint::area_fields(ui, window)
            });
            paint::fill_section(ui, window);
        }
        Mode::Shape => {
            shape::sections(ui, window);
            if window.tools.supports.auto.is_some() {
                section(ui, &tr!("supports-sizes"), None, |ui| {
                    auto_sizes(ui, window)
                });
            }
        }
    }
}

/// What stays over the scrolling blocks: the preset in hand and its groups. A plate
/// nothing was ever chosen for starts on the automatic preset.
pub fn top(ui: &mut egui::Ui, window: &mut Window) {
    let standing = standing(window.doc).0 > 0;
    let tool = &mut window.tools.supports;
    if tool.sizing().is_none() && !standing {
        tool.set_automatic(true, &window.machine.slicing.catalogue);
    }
    support_picker::line(ui, window);
}

/// What a click on the model does. Painting and blocking are one mode, chosen inside it,
/// and alt takes off whichever is in hand, so there is no eraser of its own.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Pick,
    Place,
    Area,
    Shape,
}

/// The mode is the tool underneath: Supports picks, Paint places and paints.
fn mode(window: &Window) -> Mode {
    match (*window.tool, window.tools.supports.placing) {
        (Tool::Paint, Placing::Point) => Mode::Place,
        (Tool::Paint, _) => Mode::Area,
        (Tool::Shape, _) => Mode::Shape,
        _ => Mode::Pick,
    }
}

fn set_mode(window: &mut Window, mode: Mode) {
    let supports = &mut window.tools.supports;
    match mode {
        Mode::Pick => *window.tool = Tool::Supports,
        Mode::Shape => *window.tool = Tool::Shape,
        Mode::Place => {
            *window.tool = Tool::Paint;
            supports.placing = Placing::Point;
        }
        Mode::Area => {
            *window.tool = Tool::Paint;
            if !matches!(supports.placing, Placing::Paint | Placing::Block) {
                supports.placing = Placing::Paint;
            }
        }
    }
}

/// The modes as icons at the right of the card's heading, laid right to left.
pub fn modes(ui: &mut egui::Ui, window: &mut Window) {
    let modes = [
        (Mode::Shape, icon::SHAPE, tr!("supports-mode-shape")),
        (Mode::Area, icon::PAINT, tr!("supports-mode-area")),
        (Mode::Place, icon::ADD, tr!("supports-mode-place")),
        (Mode::Pick, icon::SELECT, tr!("supports-mode-pick")),
    ];
    ui.spacing_mut().item_spacing.x = 2.0;
    let current = mode(window);
    for (value, glyph, name) in modes {
        if icon_toggle(ui, glyph, &name, current == value).clicked() {
            set_mode(window, value);
        }
    }
}

/// Where an automatic run puts supports, and the lift of the one model picked.
fn where_fields(ui: &mut egui::Ui, window: &mut Window) {
    scope::editing_supports(window, |window| {
        if window.tools.supports.auto.is_some() {
            auto_held_up(ui, window);
        } else {
            let profile = &mut window.tools.supports.profile;
            let saved =
                saved_as(&window.machine.slicing.catalogue, profile).map(|entry| &entry.profile);
            held_up(ui, profile, saved);
        }
        if let [id] = *window.doc.scene.selection() {
            lift_row(ui, &mut window.doc.scene, id);
        }
    });
}

/// How far the one picked model stands off the plate, the same number as in Position.
fn lift_row(ui: &mut egui::Ui, scene: &mut Scene, id: ObjectId) {
    let Some(object) = scene.get_mut(id) else {
        return;
    };
    let Some(mut lift_mm) = object.lift() else {
        return;
    };
    if number_row(
        ui,
        &tr!("position-lift"),
        &mut lift_mm,
        &tr!("unit-mm"),
        super::position::POSITION_DRAG,
        0.0..=super::position::MAX_LIFT_MM,
        2,
    ) {
        object.lift_to(lift_mm);
    }
}

/// What has no room, Fill painted areas while there is paint or the brush is in hand, and
/// the run: the cavity's button over Grow for a hollow model, or the run's progress.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    let (points, columns) = standing(window.doc);
    // Trees still growing stand nothing yet, which is not a want of room.
    let growing = window
        .doc
        .scene
        .targets()
        .any(crate::scene::SceneObject::supports_pending);
    if points != columns && !growing {
        let line = tr!("supports-without-room", count = points - columns);
        tone(
            ui,
            &format!("{} {line}", icon::WARNING),
            theme::colors().text_mid,
        );
    }
    if let Some(job) = window.tools.supports.job.as_ref() {
        running(ui, job);
        return;
    }
    fill(ui, window);
    cavity(ui, window);
    grow(ui, window, points);
}

fn fill(ui: &mut egui::Ui, window: &mut Window) {
    let painted = window
        .doc
        .scene
        .targets()
        .any(|object| !object.supports.painted().is_empty());
    if !painted && mode(window) != Mode::Area {
        return;
    }
    if companion_button(ui, icon::PAINT, &tr!("paint-fill"), painted).clicked() {
        let added = window
            .tools
            .supports
            .fill(&mut window.doc.scene, &window.machine.slicing.catalogue);
        window.machine.status = Status::Info(tr!("paint-filled", count = added));
    }
}

/// Grow, and at its right the bin that takes every support away on a second click.
fn grow(ui: &mut egui::Ui, window: &mut Window, points: usize) {
    let blocker = window.tools.supports.blocker(&window.doc.scene);
    let label = if points > 0 {
        tr!("supports-grow-again")
    } else {
        tr!("supports-grow")
    };
    let what = tr!("supports-grow-tooltip", scope = window.doc.scene.scope());
    let mut all = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let width = ui.available_width() - theme::PRIMARY_H - 6.0;
        let grow = ui
            .allocate_ui(egui::vec2(width, theme::PRIMARY_H), |ui| {
                primary_button(ui, icon::SUPPORTS, &label, blocker.is_none()).on_hover_text(what)
            })
            .inner;
        if grow.clicked() {
            start(window, Side::Outside);
        }
        let tip = tr!("supports-remove-all-tooltip");
        all = bin_button(ui, theme::PRIMARY_H, BinLook::Framed, &tip, points > 0);
    });
    if all {
        for object in window.doc.scene.targets_mut() {
            object.supports.clear();
        }
        window.tools.supports.picked.clear();
    }
    if let Some(reason) = blocker {
        ui.vertical_centered(|ui| tone(ui, &reason, theme::colors().text_low));
    }
}

/// Where the automatic profile chosen puts supports, against the one it ships as. Each
/// support is then sized by what it holds; see `docs/decisions/0325`.
fn auto_held_up(ui: &mut egui::Ui, window: &mut Window) {
    let Some(auto) = window.tools.supports.auto.as_mut() else {
        return;
    };
    let shipped = window
        .machine
        .slicing
        .catalogue
        .autos()
        .find(|entry| entry.profile.name == auto.name)
        .map(|entry| entry.profile.placing_with(&window.tools.supports.profile));
    let mut placing = auto.placing_with(&window.tools.supports.profile);
    held_up(ui, &mut placing, shipped.as_ref());
    auto.take_placing(&placing);
}

/// Where the automatic profile chosen moves each support to a heavier shape, by the area of
/// the island it holds and by how much of the model stands over the overhang it holds.
fn auto_sizes(ui: &mut egui::Ui, window: &mut Window) {
    let catalogue = &window.machine.slicing.catalogue;
    let Some(auto) = window.tools.supports.auto.as_mut() else {
        return;
    };
    let shipped = catalogue
        .autos()
        .find(|entry| entry.profile.name == auto.name)
        .map(|entry| entry.profile.clone());
    let shapes = auto.shapes.each_ref().map(|id| {
        catalogue
            .support(id)
            .map_or_else(|_| id.clone(), |entry| entry.profile.name.clone())
    });
    sizes(ui, auto, shipped.as_ref(), &shapes);
}

/// How many supports the plate carries, and how many of them have room to stand.
fn standing(doc: &Doc) -> (usize, usize) {
    doc.scene
        .targets()
        .fold((0, 0), |(points, columns), object| {
            (
                points + object.supports.point_count(),
                columns + object.supports.standing(),
            )
        })
}

/// What the click has picked: how much of it, which groups it belongs to, and the
/// three things that can be done to it without the mouse.
fn editing(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.supports.picked.is_empty() {
        hint(ui, &tr!("supports-pick-hint"));
        return;
    }
    describe(ui, &tr!("supports-picked-help"));

    let held = window.tools.supports.picked.clone();
    let supports = held
        .iter()
        .map(|picked| (picked.id, picked.frozen))
        .collect::<std::collections::BTreeSet<_>>();
    stats(
        ui,
        &[
            (tr!("supports-stat-parts"), held.len().to_string()),
            (tr!("supports-stat-supports"), supports.len().to_string()),
        ],
    );

    ui.add_space(4.0);
    let (mut regroup, mut regrow, mut remove) = (false, false, false);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let width = (ui.available_width() - theme::BUTTON_H - 12.0) / 2.0;
        regroup = compact_button(ui, &tr!("supports-move-to-group"), width).clicked();
        regrow = compact_button(ui, &tr!("supports-grow-these-again"), width).clicked();
        let tip = tr!("supports-remove-these");
        remove = instant_bin(ui, theme::BUTTON_H, BinLook::Quiet, &tip);
    });
    if regroup {
        let active = window.tools.supports.active;
        for (id, frozen) in &supports {
            if let Some(object) = window.doc.scene.get_mut(*id)
                && let Some(tree) = object.supports.frozen_mut(*frozen)
            {
                tree.set_group(active);
            }
        }
    }
    if regrow {
        // Highest first: thawing renumbers everything above what was taken out.
        for (id, frozen) in supports.iter().rev() {
            if let Some(object) = window.doc.scene.get_mut(*id) {
                let transform = object.transform;
                object.supports.thaw(*frozen, transform);
            }
        }
        window.tools.supports.picked.clear();
    }
    if remove {
        // Highest first: a support that goes whole renumbers everything above it.
        for (id, frozen) in supports.iter().rev() {
            let parts: Vec<Part> = held
                .iter()
                .filter(|picked| picked.id == *id && picked.frozen == *frozen)
                .map(|picked| picked.part)
                .collect();
            if let Some(object) = window.doc.scene.get_mut(*id) {
                object.supports.remove_parts(*frozen, &parts);
            }
        }
        window.tools.supports.picked.clear();
    }
}

/// Holding the cavities of the hollow models aimed at up, or, once something stands in
/// them, taking that away again; see `docs/decisions/0334`.
fn cavity(ui: &mut egui::Ui, window: &mut Window) {
    if !window
        .doc
        .scene
        .targets()
        .any(SceneObject::supported_inside)
    {
        return;
    }
    let inside = inside_count(ui.ctx(), &window.doc.scene);
    if inside == 0 {
        if secondary_button(ui, icon::SUPPORTS, &tr!("supports-grow-inside"))
            .on_hover_text(tr!("supports-grow-inside-tooltip"))
            .clicked()
        {
            start(window, Side::Inside);
        }
        return;
    }
    if secondary_button(ui, icon::REMOVE, &tr!("supports-remove-inside"))
        .on_hover_text(tr!("supports-remove-inside-tooltip", count = inside))
        .clicked()
    {
        let gone = SupportTool::clear_inside(&mut window.doc.scene);
        window.tools.supports.picked.clear();
        window.machine.status = Status::Info(tr!("supports-removed-inside", count = gone));
    }
}

/// [`SupportTool::inside_count`], asked again only when a support or the surface it is
/// asked against changed: it is a closest-point query per support.
fn inside_count(ctx: &egui::Context, scene: &Scene) -> usize {
    let mut hasher = DefaultHasher::new();
    for object in scene.targets().filter(|object| object.supported_inside()) {
        object.id.hash(&mut hasher);
        Arc::as_ptr(object.support_surface().0).hash(&mut hasher);
        let points = object.supports.points().iter().map(|point| point.contact);
        let tips = object
            .supports
            .frozen()
            .iter()
            .flat_map(SupportTree::contacts);
        for contact in points.chain(tips) {
            contact.to_array().map(f32::to_bits).hash(&mut hasher);
        }
    }
    let fingerprint = hasher.finish();
    let id = egui::Id::new("supports-inside-count");
    if let Some((cached, count)) = ctx.data(|data| data.get_temp::<(u64, usize)>(id))
        && cached == fingerprint
    {
        return count;
    }
    let count = SupportTool::inside_count(scene);
    ctx.data_mut(|data| data.insert_temp(id, (fingerprint, count)));
    count
}

/// Starts a run on one side of the wall, and says on the stage whether it could.
fn start(window: &mut Window, side: Side) {
    let started = window.tools.supports.start(
        &mut window.doc.scene,
        window.machine.slicing.layer_height_mm(),
        &window.machine.slicing.catalogue,
        side,
    );
    window
        .machine
        .status
        .report(&tr!("supports-placing"), started);
}

/// The progress bar and the cancel button of the run that is going.
fn running(ui: &mut egui::Ui, job: &SupportJob) {
    tone(ui, &job.label(), theme::colors().text_mid);
    progress_bar(ui, job.fraction());

    ui.add_enabled_ui(!job.is_cancelling(), |ui| {
        if secondary_button(ui, icon::CANCEL, &tr!("common-cancel")).clicked() {
            job.cancel();
        }
    });
}

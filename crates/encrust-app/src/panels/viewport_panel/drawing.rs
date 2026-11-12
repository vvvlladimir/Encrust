//! The plate drawn each frame, and the ghost of what a click would place or a drag carries.

use super::hollow::{blocker_ghost, channel_point_ghost, drain_ghost};
use super::marks::marked;
use super::overlays::{cut_line, draw_axis_letters};
use super::picking::{aims_elsewhere, material_under, printed_under, surface_under};
use super::support_ghost::cached_support_ghost;
use super::supports::carried_ghost;
use super::{GHOST_OPACITY, levels, picks_instead, text};
use crate::drain::Placing;
use crate::panels::{Window, section};
use crate::relief::ReliefMode;
use crate::render::{Banding, Ghost, PrintLook, SETTLED_SAMPLES, Shading, ViewportCallback};
use crate::scene::{ObjectId, Scene};
use crate::ui::theme;
use crate::viewport_input::Pointer;
use crate::workspace::Tool;
use core_geometry::PlacedHit;

/// What a click would put down under a resting cursor, drawn see-through as not placed yet.
/// Alt shows nothing: it takes away instead.
pub(super) fn ghost(
    ui: &egui::Ui,
    window: &Window,
    rect: egui::Rect,
    pointer: &Pointer,
) -> Option<Ghost> {
    if let Some(carried) = carried_ghost(window) {
        return Some(carried);
    }
    let removing = ui.input(|input| input.modifiers.alt) || picks_instead(ui);
    let line = ui.input(|input| input.modifiers.shift);
    let resting = !pointer.primary_down && !pointer.pan_button_down;
    if removing || !resting || !pointer.over_viewport {
        return None;
    }
    let cursor = pointer.position.map(|at| egui::pos2(at.x, at.y))?;
    if *window.tool == Tool::Paint && !window.tools.supports.placing.paints() {
        let (id, hit) = surface_under(window, rect, cursor)?;
        return faded(&window.doc.scene, id, || {
            cached_support_ghost(ui.ctx(), window, id, &hit, line)
        });
    }
    if *window.tool == Tool::Drain && window.tools.drain.placing != Placing::Channel {
        let (id, printed, hit) = printed_under(window, rect, cursor)?;
        return faded(&window.doc.scene, id, || {
            drain_ghost(window, id, printed, &hit)
        });
    }
    // A blocker is a volume, so it can be put inside the material through the cap of a cut.
    if *window.tool == Tool::Hollow {
        let (id, hit) = material_under(window, rect, cursor)?;
        return faded(&window.doc.scene, id, || {
            blocker_ghost(window, id, &hit, line)
        });
    }
    type Under = fn(&Window, egui::Rect, egui::Pos2) -> Option<(ObjectId, PlacedHit)>;
    type Shows = fn(&Window, ObjectId, &PlacedHit) -> Option<Ghost>;
    let (under, shows): (Under, Shows) = match *window.tool {
        Tool::Drain if window.tools.drain.placing == Placing::Channel => {
            (surface_under, channel_point_ghost)
        }
        Tool::Relief if window.tools.relief.mode == ReliefMode::Text => {
            (surface_under, text::text_ghost)
        }
        _ => return None,
    };
    let (id, hit) = under(window, rect, cursor)?;
    faded(&window.doc.scene, id, || shows(window, id, &hit))
}

/// The ghost `shows` builds on `id`, see-through, or nothing on a model outside the
/// selection: a click there only picks it.
pub(super) fn faded(
    scene: &Scene,
    id: ObjectId,
    shows: impl FnOnce() -> Option<Ghost>,
) -> Option<Ghost> {
    if aims_elsewhere(scene, Some(id)) {
        return None;
    }
    let mut ghost = shows()?;
    ghost.colour = ghost.colour.gamma_multiply(GHOST_OPACITY);
    Some(ghost)
}

/// Hands the 3D pass its frame: the models, the plate, the section and the shading.
pub(super) fn paint_plate(
    ui: &egui::Ui,
    window: &mut Window,
    rect: egui::Rect,
    ghost: Option<&Ghost>,
) {
    let textures = (*window.tool == Tool::Relief
        && window.tools.relief.mode == ReliefMode::Texture)
        .then(|| window.tools.relief.textures(&window.doc.scene));
    // What needs holding up is shown while the Supports tool is the one in hand, at the
    // angle its own profile is set to; see `docs/decisions/0034`.
    let overhang_deg = window
        .tool
        .shows_overhangs()
        .then(|| window.tools.supports.shown_overhang_deg(&window.doc.scene));
    crate::render::prime_label(ui.painter());
    ui.painter().rect_filled(rect, 0.0, theme::colors().sunken);
    let hidden_groups = window.tools.supports.hidden();
    let section_mm =
        section::cut_height(*window.mode, &window.view.section, &window.machine.preview);
    let bottom_cut_mm =
        section::floor_height(*window.mode, &window.view.section, &window.machine.preview);
    let capped = section_mm.is_some() || bottom_cut_mm.is_some();
    let height_px = rect.height() * ui.ctx().pixels_per_point();
    let levels = levels::levels(ui, window, capped, height_px);
    // Shells are hidden from the list of the Model mode, and only while it is in view.
    let stand_in = (*window.tool == Tool::Repair
        && window.tools.repair.mode == crate::repair::RepairMode::Model)
        .then(|| window.doc.repairs.shells.stand_in(&window.doc.scene))
        .flatten();
    let callback = ViewportCallback::new(
        &window.doc.scene,
        &window.doc.plate,
        &window.view.camera,
        window.view.options,
        Shading {
            overhang_deg,
            section_mm,
            bottom_cut_mm,
            banding: Banding {
                ranges: &window.machine.slicing.exposure,
                floor_mm: window.machine.slicing.band_floor_mm(),
            },
            textures: textures.as_ref(),
            cut_line: cut_line(window),
            hidden_groups: &hidden_groups,
            ghost,
            light: Some(&levels.light),
            stand_in: stand_in.as_ref().map(|(id, mesh)| (*id, mesh)),
            print_look: window.view.options.render.then(|| PrintLook {
                resin: theme::resin_swatch(window.machine.slicing.material.details.color),
                layer_height_mm: window.machine.slicing.layer_height_mm(),
                clarity: window.machine.slicing.material.details.clarity,
                depth_of_field: window.view.options.depth_of_field,
            }),
        },
        rect,
    )
    .highlighting(&marked(
        window,
        section_mm,
        (rect, ui.input(|i| i.pointer.hover_pos())),
    ))
    .timed(window.view.ui_ms, window.view.budget.report(), levels.faces)
    .settling(std::sync::Arc::clone(&window.view.settled));
    // The picture at rest sharpens a frame at a time until it has averaged enough.
    let settled = window
        .view
        .settled
        .load(std::sync::atomic::Ordering::Relaxed);
    if window.view.options.render && !levels.light.settling && settled <= SETTLED_SAMPLES {
        ui.ctx().request_repaint();
    }
    ui.painter()
        .add(egui_wgpu::Callback::new_paint_callback(rect, callback));
    draw_axis_letters(ui, window, rect);
}

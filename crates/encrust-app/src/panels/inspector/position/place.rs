//! The Select card's Place and Auto-orient sections: where the picked model stands and
//! which way up, short of typing it.

use core_geometry::Vec3;
use core_plate::{OrientGoal, OrientPreset};

use super::{MAX_LIFT_MM, POSITION_DRAG};
use crate::i18n::tr;
use crate::panels::Window;
use crate::scene::{Axis, ObjectId, Scene};
use crate::ui::{
    Segment, Segmented, describe, icon, kept, secondary_button, section, step_bar, stepped_row,
    switch,
};

/// How far a press of either button beside the lift moves the model, and the gap between
/// a model and the mirrored copy stood beside it, millimetres.
const LIFT_STEP_MM: f32 = 1.0;
const COPY_GAP_MM: f32 = 5.0;

/// Laying a face down, centring over the plate, mirroring, and the lift off it.
pub(super) fn place(ui: &mut egui::Ui, window: &mut Window, id: ObjectId) {
    let copy_id = ui.id().with("mirror-copy");
    let mut copy = kept::get(ui.ctx(), copy_id).unwrap_or(false);
    section(ui, &tr!("position-place"), None, |ui| {
        describe(ui, &tr!("position-place-help"));
        ui.columns(2, |columns| {
            lay_on_face(&mut columns[0], window);
            if secondary_button(&mut columns[1], icon::CENTRE, &tr!("position-centre")).clicked() {
                centre(&mut window.doc.scene, window.doc.plate.center(), id);
            }
        });
        mirror(ui, &mut window.doc.scene, copy);
        switch(ui, &mut copy, &tr!("position-mirror-copy"));
        lift(ui, &mut window.doc.scene, id);
    });
    kept::set(ui.ctx(), copy_id, &copy);
}

/// Lay on face, which waits for the face to be clicked; a run of Auto-orient replaces
/// the rotation when it lands, so a face laid meanwhile would be lost.
fn lay_on_face(ui: &mut egui::Ui, window: &mut Window) {
    let orient = &mut window.tools.orient;
    let label = if orient.picking_face {
        tr!("position-click-face")
    } else {
        tr!("position-lay-on-face")
    };
    let running = orient.is_running();
    let laid = ui
        .add_enabled_ui(!running, |ui| {
            secondary_button(ui, icon::LAY_ON_FACE, &label)
        })
        .inner
        .clicked();
    if laid && orient.picking_face {
        orient.stop_picking();
    } else if laid {
        orient.picking_face = true;
    }
}

fn centre(scene: &mut Scene, middle: Vec3, id: ObjectId) {
    let Some(object) = scene.get_mut(id) else {
        return;
    };
    if let Some(bounds) = object.world_bounds() {
        let offset = middle - bounds.center();
        object.transform.translation += offset * Vec3::new(1.0, 1.0, 0.0);
    }
}

/// Every flip works on the whole selection, so a plate of four does not need the same
/// button pressed four times; with `copy` it flips a copy stood beside each instead.
fn mirror(ui: &mut egui::Ui, scene: &mut Scene, copy: bool) {
    let picked = scene.selection().to_vec();
    let mut flipped = None;
    ui.columns(Axis::ALL.len(), |columns| {
        for (column, axis) in columns.iter_mut().zip(Axis::ALL) {
            let name = tr!("position-mirror-across", axis = axis.label());
            let button = secondary_button(column, icon::FLIP, axis.label());
            if button.on_hover_text(name).clicked() {
                flipped = Some(axis);
            }
        }
    });
    if let Some(axis) = flipped {
        let flipped = if copy { copies(scene, &picked) } else { picked };
        for id in &flipped {
            scene.mirror(*id, axis);
        }
    }
}

/// A copy of each of `ids` stood clear of it to the right, all of them selected.
fn copies(scene: &mut Scene, ids: &[ObjectId]) -> Vec<ObjectId> {
    let copied: Vec<ObjectId> = ids
        .iter()
        .filter_map(|id| {
            let bounds = scene.get(*id)?.world_bounds()?;
            let width = bounds.maxs.x - bounds.mins.x;
            scene.duplicate(*id, Vec3::new(width + COPY_GAP_MM, 0.0, 0.0))
        })
        .collect();
    scene.select_many(&copied);
    copied
}

/// How high the model stands over the plate, a millimetre a press. The height typed is the
/// one auto-orient stands it on supports at; nought hands that back to the support profile.
fn lift(ui: &mut egui::Ui, scene: &mut Scene, id: ObjectId) {
    let Some(object) = scene.get_mut(id) else {
        return;
    };
    let Some(now_mm) = object.lift() else {
        return;
    };
    let mut lift_mm = now_mm;
    if stepped_row(
        ui,
        &tr!("position-lift"),
        &mut lift_mm,
        &tr!("unit-mm"),
        POSITION_DRAG,
        LIFT_STEP_MM,
        0.0..=MAX_LIFT_MM,
        2,
    ) {
        object.lift_to(lift_mm);
    }
}

/// What Auto-orient searches for, and the button that runs it; pressed again with nothing
/// changed it stands the model the next good way it found.
pub(super) fn auto_orient_section(ui: &mut egui::Ui, window: &mut Window, id: ObjectId) {
    section(ui, &tr!("position-auto-orient"), None, |ui| {
        let running = window.tools.orient.is_running();
        let preset = &mut window.tools.orient.settings.preset;
        let goals = OrientGoal::ALL.into_iter().map(OrientPreset::Goal);
        let segments: Vec<Segment<'_, OrientPreset>> = std::iter::once(OrientPreset::Auto)
            .chain(goals)
            .map(|choice| Segment::new(choice, label(choice)))
            .collect();
        let width = ui.available_width();
        ui.add_enabled_ui(!running, |ui| {
            Segmented::new(&segments).width(width).show(ui, preset);
        });
        describe(ui, &hint(*preset));
        if let Some(job) = &window.tools.orient.job {
            let (what, share) = match job.counted() {
                Some((what, share)) => (what, Some(share)),
                None => (tr!("orient-looking"), None),
            };
            if step_bar(ui, &what, share, true) {
                window.tools.orient.cancel(&mut window.machine.status);
            }
        }
        let oriented = ui
            .add_enabled_ui(!running, |ui| {
                secondary_button(ui, icon::AUTO_ORIENT, &tr!("position-auto-orient"))
            })
            .inner
            .clicked();
        if oriented {
            super::auto_orient(window, Some(id));
        }
    });
}

fn label(preset: OrientPreset) -> String {
    match preset {
        OrientPreset::Goal(OrientGoal::FewestSupports) => tr!("orient-preset-supports"),
        OrientPreset::Goal(OrientGoal::LeastPeel) => tr!("orient-preset-peel"),
        OrientPreset::Goal(OrientGoal::ShortestPrint) => tr!("orient-preset-shortest"),
        OrientPreset::Auto => tr!("orient-preset-auto"),
    }
}

fn hint(preset: OrientPreset) -> String {
    match preset {
        OrientPreset::Goal(OrientGoal::FewestSupports) => tr!("orient-preset-supports-hint"),
        OrientPreset::Goal(OrientGoal::LeastPeel) => tr!("orient-preset-peel-hint"),
        OrientPreset::Goal(OrientGoal::ShortestPrint) => tr!("orient-preset-shortest-hint"),
        OrientPreset::Auto => tr!("orient-preset-auto-hint"),
    }
}

use core_joint::{JointKind, Joints, Key, Magnets, Pegs};

use core_volume::PartSettings;

use crate::cut::{CutTool, Keep};
use crate::i18n::tr;
use crate::panels::Window;
use crate::scene::{Axis, SceneObject};
use crate::status::Status;
use crate::ui::{
    Segment, Segmented, busy, count_row, describe, hint, icon, nested, number_row, primary_button,
    secondary_button, section, switch,
};

/// The most fixings one piece of a cut face takes.
const MAX_PER_PIECE: u32 = 6;

/// Millimetres per point of drag on a clearance.
const CLEARANCE_STEP: f64 = 0.01;

/// Millimetres per point of drag on the position field.
const OFFSET_STEP: f64 = 0.1;

/// Cubic millimetres per point of drag on the smallest part.
const VOLUME_STEP: f64 = 0.1;

/// Where the plane cuts the selected model, and what to do with the halves.
/// The section slider is a view of its own, neither moved by the plane nor moving it.
/// The plane's axis and where it crosses it, for the card's heading.
pub fn fact(window: &Window) -> String {
    let state = &window.tools.cut.state;
    let axis = match state.axis {
        Axis::X => "X",
        Axis::Y => "Y",
        Axis::Z => "Z",
    };
    let height = format!("{:.2}", state.height_mm);
    tr!("cut-fact", axis = axis, height = height)
}

pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let Some(id) = window.doc.scene.selected() else {
        section(ui, &tr!("cut-plane"), None, |ui| {
            hint(ui, &tr!("cut-select-model"));
        });
        return;
    };

    if let Some(object) = window.doc.scene.get(id) {
        window.tools.cut.centre_on(id, object);
    }

    section(ui, &tr!("cut-plane"), None, |ui| {
        let axes: Vec<Segment<'_, Axis>> = Axis::ALL
            .iter()
            .map(|axis| Segment::new(*axis, axis.label()))
            .collect();
        let width = ui.available_width();
        let mut axis = window.tools.cut.state.axis;
        Segmented::new(&axes).width(width).show(ui, &mut axis);
        if axis != window.tools.cut.state.axis
            && let Some(object) = window.doc.scene.get(id)
        {
            window.tools.cut.set_axis(axis, object);
        }

        let plate = &window.doc.plate;
        let across = plate.x_mm.max(plate.y_mm);
        let (label, range) = match window.tools.cut.state.axis {
            Axis::Z => (tr!("cut-height"), 0.0..=plate.z_mm),
            Axis::X | Axis::Y => (tr!("cut-offset"), -across..=across),
        };
        number_row(
            ui,
            &label,
            &mut window.tools.cut.state.height_mm,
            &tr!("unit-mm"),
            OFFSET_STEP,
            range,
            2,
        );
        let from = match window.tools.cut.state.axis {
            Axis::Z => tr!("cut-measured-from-plate"),
            Axis::X | Axis::Y => tr!("cut-measured-from-centre"),
        };
        describe(ui, &from);
    });

    section(ui, &tr!("cut-afterwards"), None, |ui| {
        let keeps: Vec<Segment<'_, Keep>> = Keep::ALL
            .iter()
            .map(|keep| Segment::new(*keep, keep_label(*keep)))
            .collect();
        let width = ui.available_width();
        Segmented::new(&keeps)
            .width(width)
            .show(ui, &mut window.tools.cut.state.keep);
        switch(ui, &mut window.tools.cut.lay_flat, &tr!("cut-lay-flat"));
        describe(ui, &tr!("cut-lay-flat-note"));
        let both = window.tools.cut.state.keep == Keep::Both;
        joints(ui, &mut window.tools.cut.state.joints, both);
    });

    section(ui, &tr!("cut-parts"), None, |ui| {
        mark(ui, &mut window.tools.cut, window.doc.scene.get(id));
        smallest(ui, &mut window.tools.cut.state.parts);
    });
}

/// Where marking a part stands on the selected model.
#[derive(Clone, Copy)]
enum Marking {
    Waiting,
    Marked,
    Unmarked,
}

/// Mark a part, which waits for a click on the model; once one is marked, the same button
/// takes the mark away.
fn mark(ui: &mut egui::Ui, cut: &mut CutTool, object: Option<&SceneObject>) {
    let marking = match (
        cut.picking_part,
        object.and_then(|object| cut.marked_on(object)),
    ) {
        (true, _) => Marking::Waiting,
        (false, Some(_)) => Marking::Marked,
        (false, None) => Marking::Unmarked,
    };
    let label = match marking {
        Marking::Waiting => tr!("cut-parts-click"),
        Marking::Marked => tr!("cut-parts-unmark"),
        Marking::Unmarked => tr!("cut-parts-mark"),
    };
    if secondary_button(ui, icon::SELECT, &label).clicked() {
        match marking {
            Marking::Waiting => cut.picking_part = false,
            Marking::Marked => cut.unmark_part(),
            Marking::Unmarked => cut.picking_part = true,
        }
    }
    describe(ui, &tr!("cut-parts-mark-note"));
}

/// The size below which a piece Split into its parts finds is a scrap.
fn smallest(ui: &mut egui::Ui, parts: &mut PartSettings) {
    number_row(
        ui,
        &tr!("cut-parts-smallest"),
        &mut parts.smallest_mm3,
        &tr!("unit-mm3"),
        VOLUME_STEP,
        0.0..=1000.0,
        1,
    );
    describe(ui, &tr!("cut-parts-smallest-note"));
}

/// Which halves stay, as the segmented choice names it.
fn keep_label(keep: Keep) -> String {
    match keep {
        Keep::Both => tr!("cut-keep-both"),
        Keep::Below => tr!("cut-keep-lower"),
        Keep::Above => tr!("cut-keep-upper"),
    }
}

/// How the halves are joined, and the numbers of the kind chosen. A half kept alone has
/// nothing to be joined to.
fn joints(ui: &mut egui::Ui, joints: &mut Joints, both: bool) {
    let mut on = joints.kind.is_some();
    let flipped = ui
        .add_enabled_ui(both, |ui| switch(ui, &mut on, &tr!("cut-joints")).changed())
        .inner;
    if flipped {
        joints.kind = on.then_some(JointKind::Dowels);
    }
    let Some(mut kind) = joints.kind.filter(|_| both) else {
        describe(ui, &tr!("cut-joints-note"));
        return;
    };
    nested(ui, |ui| {
        let kinds: Vec<Segment<'_, JointKind>> = JointKind::ALL
            .iter()
            .map(|kind| Segment::new(*kind, kind_label(*kind)))
            .collect();
        let width = ui.available_width();
        Segmented::new(&kinds).width(width).show(ui, &mut kind);
        joints.kind = Some(kind);
        describe(ui, &kind_note(kind));
        match kind {
            JointKind::Pins | JointKind::Dowels => pegs(ui, &mut joints.pegs),
            JointKind::Magnets => magnets(ui, &mut joints.magnets),
            JointKind::Key => key(ui, &mut joints.key),
        }
    });
}

fn kind_label(kind: JointKind) -> String {
    match kind {
        JointKind::Pins => tr!("cut-joint-pins"),
        JointKind::Dowels => tr!("cut-joint-dowels"),
        JointKind::Magnets => tr!("cut-joint-magnets"),
        JointKind::Key => tr!("cut-joint-key"),
    }
}

fn kind_note(kind: JointKind) -> String {
    match kind {
        JointKind::Pins => tr!("cut-joint-pins-note"),
        JointKind::Dowels => tr!("cut-joint-dowels-note"),
        JointKind::Magnets => tr!("cut-joint-magnets-note"),
        JointKind::Key => tr!("cut-joint-key-note"),
    }
}

fn per_piece(ui: &mut egui::Ui, count: &mut u32) {
    count_row(ui, &tr!("cut-per-piece"), count, "", 0.1, 1..=MAX_PER_PIECE);
}

fn millimetres(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) {
    number_row(ui, label, value, &tr!("unit-mm"), OFFSET_STEP, range, 1);
}

fn clearance(ui: &mut egui::Ui, value: &mut f32) {
    number_row(
        ui,
        &tr!("cut-clearance"),
        value,
        &tr!("unit-mm"),
        CLEARANCE_STEP,
        0.0..=1.0,
        2,
    );
}

fn pegs(ui: &mut egui::Ui, pegs: &mut Pegs) {
    per_piece(ui, &mut pegs.per_piece);
    switch(ui, &mut pegs.auto_size, &tr!("cut-auto-size"));
    if pegs.auto_size {
        describe(ui, &tr!("cut-auto-size-note"));
    } else {
        millimetres(ui, &tr!("cut-diameter"), &mut pegs.diameter_mm, 1.0..=20.0);
        millimetres(ui, &tr!("cut-length"), &mut pegs.length_mm, 1.0..=30.0);
    }
    clearance(ui, &mut pegs.clearance_mm);
    switch(ui, &mut pegs.keyed, &tr!("cut-keyed"));
    describe(ui, &tr!("cut-keyed-note"));
}

fn magnets(ui: &mut egui::Ui, magnets: &mut Magnets) {
    per_piece(ui, &mut magnets.per_piece);
    millimetres(
        ui,
        &tr!("cut-diameter"),
        &mut magnets.diameter_mm,
        1.0..=30.0,
    );
    millimetres(
        ui,
        &tr!("cut-thickness"),
        &mut magnets.thickness_mm,
        0.5..=20.0,
    );
    clearance(ui, &mut magnets.clearance_mm);
}

fn key(ui: &mut egui::Ui, key: &mut Key) {
    millimetres(ui, &tr!("cut-width"), &mut key.width_mm, 0.4..=10.0);
    millimetres(ui, &tr!("cut-height"), &mut key.height_mm, 0.5..=10.0);
    clearance(ui, &mut key.clearance_mm);
}

/// Cutting the selected model along the plane, or splitting it into the parts it is
/// already made of.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.cut.is_busy() {
        let step = window
            .tools
            .cut
            .counted()
            .map(|counted| (counted.label(), counted.share));
        let step = step.as_ref().map(|(label, share)| (label.as_str(), *share));
        if busy(ui, &tr!("cut-cutting"), step, true) {
            window.tools.cut.cancel(&mut window.machine.status);
        }
        return;
    }
    let picked = window.doc.scene.selected();
    let what = tr!("cut-replaces");
    if primary_button(ui, &tr!("cut-cut"), picked.is_some())
        .on_hover_text(&what)
        .clicked()
        && let Some(id) = picked
        && let Err(error) = window.tools.cut.start(&window.doc.scene, id)
    {
        window.machine.status = Status::Error(error.to_string());
    }
    let split = ui
        .add_enabled_ui(picked.is_some(), |ui| {
            secondary_button(ui, icon::SPLIT, &tr!("cut-split")).on_hover_text(&what)
        })
        .inner;
    if split.clicked()
        && let Some(id) = picked
        && let Err(error) = window.tools.cut.start_split(&window.doc.scene, id)
    {
        window.machine.status = Status::Error(error.to_string());
    }
}

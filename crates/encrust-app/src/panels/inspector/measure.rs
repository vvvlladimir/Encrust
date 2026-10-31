use crate::i18n::tr;
use crate::measure::{Measure, Snap, Target};
use crate::panels::Window;
use crate::state::Tools;
use crate::ui::{
    Segment, Segmented, describe, icon, icon_toggle, readings, secondary_button, section, switch,
};

/// What a click is caught by, and what the two picked points or planes are apart.
pub fn ui(ui: &mut egui::Ui, tools: &mut Tools) {
    if tools.measure.target == Target::Points {
        section(ui, &tr!("measure-snap-to"), None, |ui| {
            settings(ui, &mut tools.measure);
        });
    }
    section(ui, &tr!("measure-reading"), None, |ui| {
        let measure = &mut tools.measure;
        let read = match measure.target {
            Target::Points => points(ui, measure),
            Target::Planes => planes(ui, measure),
        };
        if !read {
            return;
        }
        if secondary_button(ui, icon::CANCEL, &tr!("measure-clear")).clicked() {
            measure.clear();
        }
    });
}

/// Points or planes, as icons in the card's heading, laid right to left.
pub fn modes(ui: &mut egui::Ui, window: &mut Window) {
    let target = &mut window.tools.measure.target;
    for (value, glyph, name) in [
        (Target::Planes, icon::PLANES, tr!("measure-planes")),
        (Target::Points, icon::POINTS, tr!("measure-points")),
    ] {
        if icon_toggle(ui, glyph, &name, *target == value).clicked() {
            *target = value;
        }
    }
}

/// What a point snaps to, and whether the span's axes are drawn.
fn settings(ui: &mut egui::Ui, measure: &mut Measure) {
    let width = ui.available_width();
    let snaps = [
        Segment::new(Snap::Corner, tr!("measure-snap-corner")),
        Segment::new(Snap::Edge, tr!("measure-snap-edge")),
        Segment::new(Snap::Free, tr!("measure-snap-free")),
    ];
    // Alt shows Free for as long as it is held; the choice underneath stays.
    let alt = ui.input(|input| input.modifiers.alt);
    let mut shown = measure.snap_now(alt);
    if Segmented::new(&snaps).width(width).show(ui, &mut shown) && !alt {
        measure.snap = shown;
    }
    describe(
        ui,
        &match shown {
            Snap::Corner => tr!("measure-snap-corner-note"),
            Snap::Edge => tr!("measure-snap-edge-note"),
            Snap::Free => tr!("measure-snap-free-note"),
        },
    );
    switch(ui, &mut measure.show_axes, &tr!("measure-show-axes"));
}

/// The span between two points, once both are picked. Returns whether there is one.
fn points(ui: &mut egui::Ui, measure: &Measure) -> bool {
    let (Some(delta), Some(distance_mm)) = (measure.delta_mm(), measure.distance_mm()) else {
        let next = match measure.start() {
            Some(_) => tr!("measure-second-point"),
            None => tr!("measure-two-points"),
        };
        describe(ui, &next);
        return false;
    };
    let mut entries = vec![
        (tr!("measure-distance"), mm(distance_mm)),
        ("dX".to_owned(), mm(delta.x)),
        ("dY".to_owned(), mm(delta.y)),
        ("dZ".to_owned(), mm(delta.z)),
    ];
    if let Some(snapped) = measure.snapped() {
        entries.push((tr!("measure-caught"), snapped.label()));
    }
    readings(ui, &entries);
    true
}

/// How two picked planes stand to each other. Returns whether both are picked.
fn planes(ui: &mut egui::Ui, measure: &Measure) -> bool {
    let Some(gap) = measure.plane_gap() else {
        let next = match measure.planes().next() {
            Some(_) => tr!("measure-second-face"),
            None => tr!("measure-two-faces"),
        };
        describe(ui, &next);
        return false;
    };
    let angle = format!("{:.1}°", gap.angle_deg);
    let centres = mm(gap.centres_mm);
    if gap.is_parallel() {
        let thickness = mm(gap.gap_mm);
        readings(
            ui,
            &[
                (tr!("measure-gap"), thickness),
                (tr!("measure-angle"), angle),
            ],
        );
    } else {
        readings(
            ui,
            &[
                (tr!("measure-angle"), angle),
                (tr!("measure-between-centres"), centres),
            ],
        );
        describe(ui, &tr!("measure-not-parallel"));
    }
    true
}

/// A length to the hundredth of a millimetre, as every reading here gives it.
fn mm(value_mm: f32) -> String {
    tr!("value-mm", value = format!("{value_mm:.2}"))
}

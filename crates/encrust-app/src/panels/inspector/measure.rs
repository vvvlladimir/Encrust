use crate::i18n::tr;
use crate::measure::{Measure, Snap, Target};
use crate::state::Tools;
use crate::ui::{
    Segment, Segmented, describe, hint, icon, secondary_button, section, stats, subheading, switch,
};

/// What a click is caught by, and what the two picked points or planes are apart.
pub fn ui(ui: &mut egui::Ui, tools: &mut Tools) {
    section(ui, &tr!("measure-settings"), None, |ui| {
        settings(ui, &mut tools.measure);
    });
    section(ui, &tr!("measure-measure"), None, |ui| {
        let measure = &mut tools.measure;
        let read = match measure.target {
            Target::Points => points(ui, measure),
            Target::Planes => planes(ui, measure),
        };
        if !read {
            return;
        }
        hint(ui, &tr!("measure-esc-clears"));
        ui.add_space(6.0);
        if secondary_button(ui, icon::CANCEL, &tr!("measure-clear")).clicked() {
            measure.clear();
        }
    });
}

/// Points or planes, what a point snaps to, and whether the span's axes are drawn.
fn settings(ui: &mut egui::Ui, measure: &mut Measure) {
    let width = ui.available_width();
    let targets = [
        Segment::new(Target::Points, tr!("measure-points")),
        Segment::new(Target::Planes, tr!("measure-planes")),
    ];
    Segmented::new(&targets)
        .width(width)
        .show(ui, &mut measure.target);
    if measure.target == Target::Planes {
        return;
    }

    subheading(ui, &tr!("measure-snap-to"));
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
    if alt {
        hint(ui, &tr!("measure-free-while-alt"));
    } else if measure.snap != Snap::Free {
        hint(ui, &tr!("measure-hold-alt"));
    }
    ui.add_space(4.0);
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
    stats(
        ui,
        &[
            (tr!("measure-distance"), mm(distance_mm)),
            ("dX".to_owned(), mm(delta.x)),
            ("dY".to_owned(), mm(delta.y)),
            ("dZ".to_owned(), mm(delta.z)),
        ],
    );
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
        stats(
            ui,
            &[
                (tr!("measure-gap"), thickness),
                (tr!("measure-angle"), angle),
            ],
        );
    } else {
        stats(
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

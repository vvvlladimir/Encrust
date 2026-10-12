use crate::measure::{Measure, Snap, Target};
use crate::state::Tools;
use crate::ui::{
    Segment, Segmented, describe, hint, icon, secondary_button, section, stats, subheading, switch,
};

/// What a click is caught by, and what the two picked points or planes are apart.
pub fn ui(ui: &mut egui::Ui, tools: &mut Tools) {
    section(ui, "Settings", None, |ui| settings(ui, &mut tools.measure));
    section(ui, "Measure", None, |ui| {
        let measure = &mut tools.measure;
        let read = match measure.target {
            Target::Points => points(ui, measure),
            Target::Planes => planes(ui, measure),
        };
        if !read {
            return;
        }
        hint(ui, "Esc clears.");
        ui.add_space(6.0);
        if secondary_button(ui, icon::CANCEL, "Clear").clicked() {
            measure.clear();
        }
    });
}

/// Points or planes, what a point snaps to, and whether the span's axes are drawn.
fn settings(ui: &mut egui::Ui, measure: &mut Measure) {
    let width = ui.available_width();
    let targets = [
        Segment::new(Target::Points, "Points"),
        Segment::new(Target::Planes, "Planes"),
    ];
    Segmented::new(&targets)
        .width(width)
        .show(ui, &mut measure.target);
    if measure.target == Target::Planes {
        return;
    }

    subheading(ui, "Snap to");
    let snaps = [
        Segment::new(Snap::Corner, "Corner"),
        Segment::new(Snap::Edge, "Edge"),
        Segment::new(Snap::Free, "Free"),
    ];
    // Alt shows Free for as long as it is held; the choice underneath stays.
    let alt = ui.input(|input| input.modifiers.alt);
    let mut shown = measure.snap_now(alt);
    if Segmented::new(&snaps).width(width).show(ui, &mut shown) && !alt {
        measure.snap = shown;
    }
    describe(
        ui,
        match shown {
            Snap::Corner => "The nearest corner where the surface turns catches a click.",
            Snap::Edge => "The nearest edge where the surface turns catches a click.",
            Snap::Free => "Exactly where the click lands.",
        },
    );
    if alt {
        hint(ui, "Free while Alt is held.");
    } else if measure.snap != Snap::Free {
        hint(ui, "Hold Alt to measure freely.");
    }
    ui.add_space(4.0);
    switch(ui, &mut measure.show_axes, "Show dX, dY, dZ in the view");
}

/// The span between two points, once both are picked. Returns whether there is one.
fn points(ui: &mut egui::Ui, measure: &Measure) -> bool {
    let (Some(delta), Some(distance_mm)) = (measure.delta_mm(), measure.distance_mm()) else {
        let next = match measure.start() {
            Some(_) => "Click the second point.",
            None => "Click two points on a model.",
        };
        describe(ui, next);
        return false;
    };
    stats(
        ui,
        &[
            ("Distance", format!("{distance_mm:.2} mm")),
            ("dX", format!("{:.2} mm", delta.x)),
            ("dY", format!("{:.2} mm", delta.y)),
            ("dZ", format!("{:.2} mm", delta.z)),
        ],
    );
    true
}

/// How two picked planes stand to each other. Returns whether both are picked.
fn planes(ui: &mut egui::Ui, measure: &Measure) -> bool {
    let Some(gap) = measure.plane_gap() else {
        let next = match measure.planes().next() {
            Some(_) => "Click the second flat face.",
            None => "Click two flat faces on a model.",
        };
        describe(ui, next);
        return false;
    };
    let angle = format!("{:.1}°", gap.angle_deg);
    let centres = format!("{:.2} mm", gap.centres_mm);
    if gap.is_parallel() {
        let thickness = format!("{:.2} mm", gap.gap_mm);
        stats(ui, &[("Gap", thickness), ("Angle", angle)]);
    } else {
        stats(ui, &[("Angle", angle), ("Between centres", centres)]);
        describe(ui, "Not parallel, so there is no one gap between them.");
    }
    true
}

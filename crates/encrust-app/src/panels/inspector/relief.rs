use crate::panels::Window;
use crate::state::Doc;
use crate::ui::{
    describe, hint, icon, later, meta, number_row, primary_button, progress_bar, secondary_button,
    section, stats, theme, tone,
};

/// Per point of drag: a relief is measured in tenths of a millimetre, precision in
/// hundredths of its own range.
const DEPTH_STEP: f64 = 0.05;
const FINE_STEP: f64 = 0.01;

/// Past two millimetres a relief is a shape of its own rather than a texture, and it
/// would cost a band that wide on every voxel of the field.
const MAX_DEPTH_MM: f32 = 2.0;

/// The texture the models on the plate came with, and how deep it is pressed into them.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, "Image", None, |ui| {
        describe(
            ui,
            "A model's own texture becomes geometry: white moves the surface out, black \
             leaves it where it is.",
        );
        carried(ui, window.doc);
        // TODO(step-8): a greyscale image chosen from a file, shown here as a thumbnail.
        later(ui, |ui| secondary_button(ui, icon::OPEN, "Choose"));
    });
    section(ui, "Depth", None, |ui| {
        depth(ui, window);
        // TODO(step-8): tile the image over the surface, at a size and a turn of its own.
        later(ui, |ui| {
            let mut tile_mm = 12.0_f32;
            number_row(
                ui,
                "Tile size",
                &mut tile_mm,
                "mm",
                DEPTH_STEP,
                1.0..=100.0,
                1,
            );
            let mut angle_deg = 0.0_f32;
            number_row(ui, "Angle", &mut angle_deg, "\u{b0}", 1.0, 0.0..=360.0, 0);
        });
    });
}

fn depth(ui: &mut egui::Ui, window: &mut Window) {
    number_row(
        ui,
        "Depth",
        &mut window.tools.relief.state.amplitude_mm,
        "mm",
        DEPTH_STEP,
        -MAX_DEPTH_MM..=MAX_DEPTH_MM,
        2,
    );
    number_row(
        ui,
        "Precision",
        &mut window.tools.relief.state.precision,
        "",
        FINE_STEP,
        0.0..=1.0,
        2,
    );

    let lattice_mm = window.tools.relief.settings().voxel_mm(
        window
            .doc
            .scene
            .targets()
            .map(|o| o.mesh.surface_area())
            .fold(0.0_f32, f32::max),
    );
    describe(
        ui,
        &format!(
            "The relief is cut on a {lattice_mm:.2} mm lattice, and the whole model is \
             meshed again at it. A negative depth sinks the texture in.",
        ),
    );
}

/// Which of the models on the plate still carry a texture to press.
fn carried(ui: &mut egui::Ui, doc: &Doc) {
    let mut mapped: Vec<&str> = doc
        .scene
        .targets()
        .filter_map(|object| object.mapped.as_ref())
        .flat_map(|mapped| mapped.names.iter().map(String::as_str))
        .collect();
    mapped.sort_unstable();
    mapped.dedup();

    let [first, rest @ ..] = mapped.as_slice() else {
        tone(
            ui,
            "No texture on the plate. It is read from the file a model is opened from, \
             and is gone once it has been pressed in.",
            theme::colors().text_low,
        );
        return;
    };

    let label = match rest.len() {
        0 => "Texture".to_owned(),
        more => format!("{} textures", more + 1),
    };
    stats(ui, &[(label.as_str(), truncated(first))]);
    // The rest are a list rather than a second cell: a file name is long enough that two
    // to a row would be two ellipses.
    meta(
        ui,
        &rest.iter().map(|name| truncated(name)).collect::<Vec<_>>(),
    );
}

/// How wide a file name may be before its middle is cut out. A panel is narrow, and the
/// end of an image's name is what tells two of them apart.
const NAME_CHARS: usize = 22;

fn truncated(name: &str) -> String {
    let count = name.chars().count();
    if count <= NAME_CHARS {
        return name.to_owned();
    }
    let keep = NAME_CHARS / 2 - 1;
    let head: String = name.chars().take(keep).collect();
    let tail: String = name.chars().skip(count - keep).collect();
    format!("{head}…{tail}")
}

/// Pressing the texture in, or the progress of the run that is going.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.relief.is_busy() {
        tone(ui, "Pressing...", theme::colors().text_mid);
        progress_bar(ui, None);
        return;
    }

    let blocked = window.tools.relief.blocker(&window.doc.scene);
    if primary_button(ui, icon::RELIEF, "Press it in", blocked.is_none()).clicked() {
        let started = window.tools.relief.start(&window.doc.scene);
        window.machine.status.report("Pressing the relief", started);
    }

    let scope = format!(
        "Presses the texture into {}. The model is replaced, so its supports and its \
         cavity go with it.",
        window.doc.scene.scope()
    );
    hint(ui, blocked.unwrap_or(&scope));
}

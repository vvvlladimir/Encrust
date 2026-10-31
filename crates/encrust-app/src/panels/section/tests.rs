use super::playback::{fraction, run_up};
use super::reading::Reading;
use super::track::{grabs_floor, marks_of};
use super::*;
use crate::preview::Preview;
use crate::ui::{LayerMark, LayerMarks, span_fraction};
use crate::workspace::{Mode, Section};

/// BUG: typing a layer pulled the track and its knob up over the readout.
#[test]
fn typing_a_layer_moves_nothing_under_the_readout() {
    let reading = Reading {
        at: Some(0.5),
        floor: None,
        layer: 12,
        layers: 24,
        height_mm: 0.6,
        top_mm: 1.2,
    };
    let below = |typing: bool| {
        let ctx = egui::Context::default();
        crate::ui::theme::apply(&ctx);
        if typing {
            ctx.data_mut(|data| data.insert_temp(egui::Id::new("layer-readout"), "7".to_owned()));
        }
        let mut cursor = 0.0;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            reading.show(ui);
            cursor = ui.cursor().top();
        });
        output.textures_delta.clear();
        cursor
    };
    assert_eq!(
        below(true),
        below(false),
        "the box stands inside the readout"
    );
}

#[test]
fn the_strip_marks_the_widest_layer_full_height_and_an_island_where_it_starts() {
    use core_analysis::{Measured, cure};
    use core_raster::{LayerRuns, PixelPitch};

    // Four layers on a 10 x 10 panel of 1 mm pixels: a pixel, a row, a pixel, and a
    // pixel standing on nothing.
    let layer = |lit: &[u32]| {
        let mut builder = LayerRuns::builder(10, 10);
        for &at in lit {
            builder.pad_to(at);
            builder.push(1, 255);
        }
        cure(&builder.finish(), PixelPitch { x: 1.0, y: 1.0 })
    };
    let mut measured = Measured::new(0);
    for lit in [&[0][..], &[0, 1, 2, 3], &[0], &[55]] {
        measured.push(layer(lit), 0.05);
    }

    let marks = marks_of(&measured, 4);
    assert_eq!(marks.widths, vec![0.25, 1.0, 0.25, 0.25]);
    assert_eq!(marks.risks.len(), 1);
    assert!((marks.risks[0].at - 1.0).abs() < 1e-6, "the top layer");
}

#[test]
fn a_press_on_a_risk_bar_takes_the_nearest_risk_and_misses_beside_the_bar() {
    let mark = |at: f32, layer| LayerMark {
        at,
        tint: egui::Color32::RED,
        layer,
    };
    let marks = LayerMarks {
        widths: Vec::new(),
        risks: vec![mark(0.10, 10), mark(0.12, 12), mark(0.50, 50)],
    };
    // A 200 point track from y 0 at its top: a share `at` up stands at 200 * (1 - at).
    let span = egui::Rangef::new(0.0, 200.0);
    let at = |x: f32, y: f32| marks.under(span, 50.0, egui::pos2(x, y));
    assert_eq!(
        at(50.0, 176.5),
        Some(1),
        "nearer 0.12 at y 176 than 0.10 at y 180"
    );
    assert_eq!(at(50.0, 100.0), Some(2), "0.50 stands at y 100");
    assert_eq!(at(50.0, 140.0), None, "no bar within reach");
    assert_eq!(at(90.0, 100.0), None, "beside the bar");
}

#[test]
fn the_prepare_mode_cuts_where_the_strip_was_left() {
    let section = Section {
        height_mm: Some(3.5),
        floor_mm: None,
        playing: false,
    };
    let preview = Preview::default();
    assert_eq!(
        cut_height(Mode::Prepare, &section, &preview),
        Some(3.5),
        "the prepare mode carries its own height"
    );
}

#[test]
fn the_preview_mode_cuts_at_the_layer_it_is_showing() {
    // An empty preview is showing no layer, so it asks for no cut whatever the
    // prepare mode was left at.
    let section = Section {
        height_mm: Some(3.5),
        floor_mm: None,
        playing: false,
    };
    assert_eq!(
        cut_height(Mode::Preview, &section, &Preview::default()),
        None
    );
}

#[test]
fn the_floor_never_rises_past_the_top_cut() {
    let section = |height_mm, floor_mm| Section {
        height_mm,
        floor_mm,
        playing: false,
    };
    let preview = Preview::default();
    let floor = |section| floor_height(Mode::Prepare, &section, &preview);
    assert_eq!(floor(section(None, None)), None, "no floor cuts nothing");
    assert_eq!(floor(section(None, Some(2.0))), Some(2.0));
    assert_eq!(floor(section(Some(5.0), Some(2.0))), Some(2.0));
    assert_eq!(
        floor(section(Some(1.0), Some(2.0))),
        Some(1.0),
        "a top cut moved under the floor takes it down with it"
    );
    assert_eq!(
        floor_height(Mode::Preview, &section(None, Some(2.0)), &preview),
        None,
        "an empty preview has no layers to cut from below"
    );
}

#[test]
fn a_press_takes_the_nearer_handle_and_the_top_where_they_meet() {
    // The top handle at y = 100, the floor at y = 300; y grows downwards.
    assert!(!grabs_floor(150.0, 100.0, 300.0));
    assert!(grabs_floor(250.0, 100.0, 300.0));
    assert!(!grabs_floor(200.0, 100.0, 300.0), "halfway is the top's");
    assert!(!grabs_floor(195.0, 200.0, 200.0), "over both is the top");
    assert!(grabs_floor(205.0, 200.0, 200.0), "under both is the floor");
}

#[test]
fn a_cut_counts_the_layers_of_the_model_under_it() {
    // Ten millimetres at half a millimetre a layer is twenty layers.
    assert_eq!(
        layer_at(10.0, (0.0, 10.0), 0.5),
        (20, 20),
        "the top is the last"
    );
    assert_eq!(
        layer_at(2.6, (0.0, 10.0), 0.5),
        (5, 20),
        "nearest layer, not past it"
    );
    assert_eq!(
        layer_at(-1.0, (0.0, 10.0), 0.5),
        (0, 20),
        "under the plate is none"
    );
    assert_eq!(
        layer_at(5.0, (5.0, 5.0), 0.5),
        (0, 0),
        "a flat model has no layers"
    );
}

#[test]
fn a_typed_layer_stands_that_many_layers_up_and_the_top_shows_everything() {
    assert_eq!(height_of_layer(4, (1.0, 10.0), 0.5), Some(3.0));
    assert_eq!(
        height_of_layer(0, (1.0, 10.0), 0.5),
        Some(1.0),
        "layer zero is the bottom"
    );
    assert_eq!(
        height_of_layer(18, (1.0, 10.0), 0.5),
        None,
        "the top cuts nothing away"
    );
    assert_eq!(
        height_of_layer(900, (1.0, 10.0), 0.5),
        None,
        "past the top is the top"
    );
}

#[test]
fn the_track_reads_the_pointer_from_its_bottom_end_and_clamps_past_either() {
    let span = egui::Rangef::new(100.0, 300.0);
    assert_eq!(span_fraction(span, 200.0), 0.5);
    assert_eq!(span_fraction(span, 300.0), 0.0, "the bottom is the plate");
    assert_eq!(span_fraction(span, 0.0), 1.0, "over the top is the top");
    assert_eq!(span_fraction(span, 900.0), 0.0);
    assert_eq!(
        span_fraction(egui::Rangef::new(5.0, 5.0), 5.0),
        1.0,
        "a track with no length stands at the top"
    );
}

#[test]
fn the_cut_runs_up_the_model_and_stops_at_the_top() {
    let range = (0.0, 10.0);
    assert_eq!(
        run_up(None, range, 1.0),
        Some(0.0),
        "a cut parked at the top starts again from the bottom"
    );
    assert_eq!(run_up(Some(4.0), range, 1.0), Some(5.0));
    assert_eq!(
        run_up(Some(9.5), range, 1.0),
        None,
        "past the top the whole model is shown again, and the play stops"
    );
}

#[test]
fn a_range_of_no_height_reads_as_full() {
    assert_eq!(fraction(5.0, 5.0, 5.0), 1.0);
    assert_eq!(fraction(0.0, 10.0, 2.5), 0.25);
    assert_eq!(
        fraction(0.0, 10.0, 40.0),
        1.0,
        "a value past the top clamps"
    );
}

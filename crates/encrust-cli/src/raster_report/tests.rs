use core_analysis::{Cured, Stretch, cure};
use core_raster::{Grey, LayerRuns, PixelPitch};

use super::*;

/// One layer into the report, the way `stream_into` puts it there: folded for what it
/// cures, then counted for what it cost the panel. The stack is 0.05 mm a layer.
fn record(report: &mut RasterReport, overflow_px: f32, layer: Cured) -> Vec<Stretch> {
    let taken = report.measured.push(layer, 0.05);
    report.count(overflow_px);
    taken
}

fn report() -> RasterReport {
    RasterReport::new(
        PathBuf::from("out"),
        RasterSettings {
            width_px: 8520,
            height_px: 4320,
            pitch: PixelPitch { x: 0.018, y: 0.018 },
            mirror_x: true,
            mirror_y: false,
            shading: Shading::Coverage,
            grey: Grey::default(),
            blur_px: 0,
        },
        Measured::new(0),
    )
}

/// A square `side` pixels across in the corner of a panel of 1 mm pixels.
fn square(side: u32) -> Cured {
    let mut builder = LayerRuns::builder(100, 100);
    for row in 0..side {
        builder.pad_to(row * 100);
        builder.push(side, 255);
    }
    cure(&builder.finish(), PixelPitch { x: 1.0, y: 1.0 })
}

#[test]
fn a_stack_that_fits_reports_no_defect() {
    let mut report = report();
    let _ = record(&mut report, 0.0, square(0));
    let _ = record(&mut report, 0.0, square(0));

    let text = report.to_string();
    assert!(report.is_clean());
    assert!(text.contains("masks         2 written to out"), "{text}");
    assert!(text.contains("coverage, exact pixel area"));
    assert!(!text.contains("raster defect"));
}

#[test]
fn clipping_is_counted_and_the_worst_overflow_kept() {
    let mut report = report();
    let _ = record(&mut report, 0.0, square(0));
    let _ = record(&mut report, 4.5, square(0));
    let _ = record(&mut report, 1.5, square(0));

    assert!(!report.is_clean());
    assert_eq!(report.clipped_layers, 2);
    assert!(
        report
            .to_string()
            .contains("raster defect 2 layers clipped, up to 4.5 px past the panel")
    );
}

#[test]
fn the_volume_and_the_hardest_layer_come_from_the_masks() {
    // Three layers of 0.05 mm over 9 + 25 + 16 mm^2.
    let mut report = report();
    for side in [3, 5, 4] {
        let _ = record(&mut report, 0.0, square(side));
    }
    assert!((report.measured.volume_mm3() - 2.5).abs() < 1e-4);
    let text = report.to_string();
    assert!(text.contains("hardest pull  layer 2"), "{text}");
    assert!(text.contains("widest step   layer 2, 16.0 mm^2"), "{text}");
    assert!(
        !text.contains("risk"),
        "a pyramid standing on itself: {text}"
    );
}

#[test]
fn an_island_is_named_with_its_layer_and_the_layer_it_fails_on() {
    let mut report = report();
    let _ = record(&mut report, 0.0, square(3));
    let mut builder = LayerRuns::builder(100, 100);
    builder.pad_to(50 * 100 + 50);
    builder.push(4, 255);
    let _ = record(
        &mut report,
        0.0,
        cure(&builder.finish(), PixelPitch { x: 1.0, y: 1.0 }),
    );

    let text = report.to_string();
    assert!(
        text.contains("risk          layer 2, island of 4.00 mm^2"),
        "{text}"
    );
    assert!(text.contains("likely fails  at layer 2"), "{text}");
}

#[test]
fn an_island_taken_out_is_counted_rather_than_named() {
    let mut report = RasterReport {
        measured: Measured::new(0).removing_islands(),
        ..report()
    };
    let _ = record(&mut report, 0.0, square(3));
    let mut builder = LayerRuns::builder(100, 100);
    builder.pad_to(50 * 100 + 50);
    builder.push(4, 255);
    let taken = record(
        &mut report,
        0.0,
        cure(&builder.finish(), PixelPitch { x: 1.0, y: 1.0 }),
    );

    assert_eq!(taken.len(), 1, "the island's one row");
    let text = report.to_string();
    assert!(
        text.contains("islands       1 taken out of the file, over 1 layers"),
        "{text}"
    );
    assert!(!text.contains("risk"), "{text}");
}

#[test]
fn binary_shading_says_so() {
    let mut report = RasterReport::new(
        PathBuf::from("out"),
        RasterSettings {
            shading: Shading::Binary,
            ..report().settings
        },
        Measured::new(0),
    );
    let _ = record(&mut report, 0.0, square(0));
    assert!(report.to_string().contains("binary, no intermediate grey"));
}

#[test]
fn a_blur_is_named_with_its_radius() {
    let settings = RasterSettings {
        blur_px: 2,
        ..report().settings
    };
    assert_eq!(
        describe_shading(&settings),
        "coverage, exact pixel area, all 255 greys, blurred 2 px"
    );
}

#[test]
fn a_grey_range_reads_as_its_two_ends() {
    assert_eq!(
        parse_grey_range("127..255"),
        Ok(GreyRange { min: 127, max: 255 })
    );
    assert!(parse_grey_range("200..100").is_err(), "inverted");
    assert!(parse_grey_range("0..256").is_err(), "past white");
    assert!(parse_grey_range("127").is_err(), "one end");
}

#[test]
fn a_narrowed_range_is_named_with_its_ends() {
    let mut settings = report().settings;
    settings.grey.range = GreyRange { min: 127, max: 255 };
    assert_eq!(
        describe_shading(&settings),
        "coverage, exact pixel area, all 255 greys spread over 127..255"
    );
}

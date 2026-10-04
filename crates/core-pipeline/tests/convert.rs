//! A sliced file read back and written again in another container, through the public
//! entry points alone.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::io::Cursor;
use std::path::Path;

use core_format::{FormatError, LayerEntry, OpenFile, SlicedFile};
use core_pipeline::{Converting, PipelineError, SlicedFormat, convert, convert_to, open};
use core_raster::Run;
use printer_profiles::{MaterialProfile, PrinterProfile};

const WIDTH_PX: u32 = 8;
const HEIGHT_PX: u32 = 4;

fn printer(width_px: u32) -> PrinterProfile {
    let toml = format!(
        r#"
name = "Convert"
manufacturer = "Test"

[display]
width_px = {width_px}
height_px = {HEIGHT_PX}
width_mm = 0.8
height_mm = 0.4

[build_volume]
x = 0.8
y = 0.4
z = 10.0
"#
    );
    PrinterProfile::from_toml_str(&toml, Path::new("inline.toml"))
        .expect("the inline profile is valid")
}

/// A file somebody else wrote, held in memory: a header and the runs of each layer.
struct Written {
    facts: SlicedFile,
    layers: Vec<Vec<Run>>,
    /// A layer that cannot be decoded, for the error a broken file gives.
    broken: Option<u32>,
}

impl OpenFile for Written {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        if self.broken == Some(index) {
            return Err(FormatError::NoSuchLayer {
                index,
                layer_count: 0,
            });
        }
        Ok(self.layers[index as usize].clone())
    }
}

/// Four 0.05 mm layers, the bottom one exposed for 20 s and the rest for 2 s but the
/// third, which takes 3 s; each lights a different number of pixels in full.
fn written() -> Written {
    let exposures = [20.0, 2.0, 3.0, 2.0];
    let layers: Vec<Vec<Run>> = (1..=4)
        .map(|lit| {
            vec![
                Run {
                    length: lit,
                    value: 255,
                },
                Run {
                    length: WIDTH_PX * HEIGHT_PX - lit,
                    value: 0,
                },
            ]
        })
        .collect();
    Written {
        facts: SlicedFile {
            format: "test",
            version: None,
            machine: None,
            slicer: None,
            resin: None,
            width_px: WIDTH_PX,
            height_px: HEIGHT_PX,
            display_mm: Some((0.8, 0.4)),
            layer_height_mm: 0.05,
            exposure_s: 2.0,
            bottom_exposure_s: 20.0,
            bottom_layers: 1,
            print_time_s: None,
            volume_mm3: None,
            grey_steps: 256,
            layers: exposures
                .iter()
                .enumerate()
                .map(|(index, &exposure_s)| LayerEntry {
                    z_mm: 0.05 * (index + 1) as f32,
                    exposure_s,
                    offset: 0,
                    size: 0,
                })
                .collect(),
        },
        layers,
        broken: None,
    }
}

fn converting<'a>(printer: &'a PrinterProfile, material: &'a MaterialProfile) -> Converting<'a> {
    Converting {
        format: SlicedFormat::Goo,
        name: "converted",
        printer,
        material,
        created_unix_s: 0,
    }
}

#[test]
fn every_mask_and_exposure_survives_a_conversion() {
    let mut source = written();
    let mut sink = Cursor::new(Vec::new());
    let resin = MaterialProfile::default();
    let printer = printer(WIDTH_PX);
    let converted = convert_to(
        &mut source,
        &converting(&printer, &resin),
        &mut sink,
        &mut (),
    )
    .expect("the file converts")
    .expect("nothing cancelled it");

    // One to four lit pixels of 0.1 x 0.1 mm, each 0.05 mm deep: 10 voxels of 0.0005 mm3.
    assert_eq!(converted.layers, 4);
    assert!((converted.volume_mm3 - 0.005).abs() < 1e-6, "{converted:?}");

    sink.set_position(0);
    let mut back = open(Path::new("converted.goo"), sink).expect("the new file reads");
    let facts = back.facts().clone();
    assert_eq!(facts.layer_count(), 4);
    assert!((facts.exposure_s - 2.0).abs() < 1e-6);
    assert!((facts.bottom_exposure_s - 20.0).abs() < 1e-6);
    assert!(
        (facts.layers[2].exposure_s - 3.0).abs() < 1e-6,
        "the third layer keeps its own exposure, got {}",
        facts.layers[2].exposure_s
    );
    assert!((facts.layers[3].exposure_s - 2.0).abs() < 1e-6);
    for index in 0..4 {
        assert_eq!(
            back.layer(index).expect("a layer decodes"),
            source.layers[index as usize],
            "layer {index} is the mask it was"
        );
    }
}

#[test]
fn a_panel_of_another_size_is_refused() {
    let resin = MaterialProfile::default();
    let printer = printer(WIDTH_PX * 2);
    let error = convert_to(
        &mut written(),
        &converting(&printer, &resin),
        &mut Cursor::new(Vec::new()),
        &mut (),
    )
    .expect_err("an 8 px mask on a 16 px panel");
    assert!(matches!(
        error,
        PipelineError::PanelMismatch {
            file_px: (8, 4),
            printer_px: (16, 4)
        }
    ));
}

#[test]
fn a_panel_of_the_same_resolution_but_another_size_is_refused() {
    let mut source = written();
    source.facts.display_mm = Some((1.6, 0.8));
    let resin = MaterialProfile::default();
    let printer = printer(WIDTH_PX);
    let error = convert_to(
        &mut source,
        &converting(&printer, &resin),
        &mut Cursor::new(Vec::new()),
        &mut (),
    )
    .expect_err("masks drawn for 0.2 mm pixels on a panel of 0.1 mm pixels");
    assert!(matches!(error, PipelineError::PanelSizeMismatch { .. }));
}

#[test]
fn a_stack_of_varying_heights_is_refused() {
    let mut source = written();
    source.facts.layers[3].z_mm = 0.25;
    let resin = MaterialProfile::default();
    let printer = printer(WIDTH_PX);
    let error = convert_to(
        &mut source,
        &converting(&printer, &resin),
        &mut Cursor::new(Vec::new()),
        &mut (),
    )
    .expect_err("the last layer is 0.1 mm");
    assert!(matches!(error, PipelineError::VaryingHeights));
}

#[test]
fn a_layer_that_does_not_decode_fails_the_file_and_leaves_none() {
    let mut source = Written {
        broken: Some(2),
        ..written()
    };
    let path = std::env::temp_dir().join(format!("encrust-convert-{}.goo", std::process::id()));
    let resin = MaterialProfile::default();
    let printer = printer(WIDTH_PX);
    let error = convert(&mut source, &converting(&printer, &resin), &path, &mut ())
        .expect_err("layer 2 is broken");

    assert!(matches!(error, PipelineError::Decode { layer: 2, .. }));
    assert!(!path.exists(), "a file stopped half way is removed");
}

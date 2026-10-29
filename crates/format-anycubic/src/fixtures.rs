//! Sample profiles the writer tests build jobs from.

use std::io::Cursor;
use std::path::Path;

use core_format::{ExposurePlan, LayerPlan, LayerSink, PrintJob, SlicedFileWriter};
use core_raster::{Grey, LayerRuns, PixelPitch, RasterSettings, Shading};
use printer_profiles::{MaterialProfile, PrinterProfile};

pub(crate) fn sample_printer() -> PrinterProfile {
    PrinterProfile::from_toml_str(
        r#"
name = "Test"
manufacturer = "Test"

[display]
width_px = 8
height_px = 4
width_mm = 0.8
height_mm = 0.4

[build_volume]
x = 0.8
y = 0.4
z = 10.0
"#,
        Path::new("inline.toml"),
    )
    .expect("valid profile")
}

pub(crate) fn sample_raster() -> RasterSettings {
    RasterSettings {
        width_px: 8,
        height_px: 4,
        pitch: PixelPitch { x: 0.1, y: 0.1 },
        mirror_x: false,
        mirror_y: false,
        shading: Shading::Coverage,
        grey: Grey::default(),
        blur_px: 0,
    }
}

pub(crate) fn sample_job(layer_count: u32) -> PrintJob {
    PrintJob {
        printer: sample_printer(),
        material: MaterialProfile::default(),
        raster: sample_raster(),
        plan: LayerPlan::of_count(
            MaterialProfile::default().layer_height_mm,
            layer_count as usize,
        ),
        volume_mm3: 1000.0,
        exposure: ExposurePlan::default(),
        thumbnail: None,
        created_unix_s: 0,
    }
}

/// An 8 x 4 layer with a lit block from column 2 to 5 in rows 1 and 2.
pub(crate) fn lit_block() -> LayerRuns {
    let mut builder = LayerRuns::builder(8, 4);
    builder.push(10, 0);
    builder.push(4, 255);
    builder.push(4, 0);
    builder.push(4, 255);
    builder.push(10, 0);
    builder.finish()
}

/// `job` written by `writer`, each of its layers `lit_block`, to a resin volume of 1500 mm³.
pub(crate) fn written_by<W: SlicedFileWriter>(writer: &W, job: &PrintJob) -> Vec<u8> {
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = writer.begin(job, &mut file).expect("the job is sound");
        for _ in 0..job.layer_count() {
            sink.push(<W::Sink<'_> as LayerSink>::encode(&lit_block()))
                .expect("a layer");
        }
        sink.finish(1500.0).expect("every promised layer arrived");
    }
    file.into_inner()
}

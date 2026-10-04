//! Sample profiles the unit tests build jobs from.

use std::path::Path;

use core_format::{ExposurePlan, LayerPlan, PrintJob};
use core_raster::{Grey, PixelPitch, RasterSettings, Shading};
use printer_profiles::{MaterialProfile, PrinterProfile};

pub(crate) fn sample_job(layer_count: u32) -> PrintJob {
    let printer = PrinterProfile::from_toml_str(
        r#"
name = "Bene Test 4"
manufacturer = "Nova3D"

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
    .expect("valid profile");

    PrintJob {
        printer,
        material: MaterialProfile::default(),
        raster: RasterSettings {
            width_px: 8,
            height_px: 4,
            pitch: PixelPitch { x: 0.1, y: 0.1 },
            mirror_x: false,
            mirror_y: false,
            shading: Shading::Coverage,
            grey: Grey::default(),
            blur_px: 0,
        },
        plan: LayerPlan::of_count(
            MaterialProfile::default().layer_height_mm,
            layer_count as usize,
        ),
        volume_mm3: 0.0,
        exposure: ExposurePlan::default(),
        thumbnail: None,
        created_unix_s: 0,
    }
}

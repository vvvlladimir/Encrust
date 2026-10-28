//! Which edge of the plate a written file starts its rows at, per machine and format (ADR 0366).
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::io::Cursor;
use std::path::Path;

use core_analysis::Measured;
use core_format::{ExposurePlan, OpenFile, PrintJob};
use core_geometry::{Mesh, Vec3};
use core_pipeline::{
    Converting, PanelOverrides, SlicedFormat, Writing, convert_to, open, raster_settings, write_to,
};
use core_slicer::{ONE_SAMPLE, SliceSettings, Windows};
use printer_profiles::{MaterialProfile, PrinterProfile};

const WIDTH_PX: u32 = 64;
const HEIGHT_PX: u32 = 32;

/// Panel of 64 x 32 pixels at a 0.2 mm pitch.
fn printer(mirror_x: bool) -> PrinterProfile {
    let toml = format!(
        r#"
name = "Orientation"
manufacturer = "Test"
mirror_x = {mirror_x}

[display]
width_px = {WIDTH_PX}
height_px = {HEIGHT_PX}
width_mm = 12.8
height_mm = 6.4

[build_volume]
x = 12.8
y = 6.4
z = 10.0
"#
    );
    PrinterProfile::from_toml_str(&toml, Path::new("inline.toml"))
        .expect("the inline profile is valid")
}

/// A slab along the near edge of the plate: rows 2 to 7 counted from that edge.
fn near_edge_slab() -> Mesh {
    core_geometry::cuboid(Vec3::new(2.0, 0.4, 0.0), Vec3::new(10.0, 1.6, 1.0))
}

/// The first and last row of the file's first layer that hold a lit pixel.
fn lit_rows(printer: PrinterProfile, format: SlicedFormat, name: &str) -> (u32, u32) {
    lit_rows_of(written(printer, format), name)
}

/// The slab written as `format` for `printer`.
fn written(printer: PrinterProfile, format: SlicedFormat) -> Vec<u8> {
    let mesh = near_edge_slab();
    let settings = raster_settings(&printer, PanelOverrides::default());
    let windows = Windows::new(
        &mesh,
        SliceSettings {
            layer_height: 0.5,
            samples: ONE_SAMPLE,
        },
        8,
    )
    .expect("a closed box slices");
    let job = PrintJob {
        printer,
        material: MaterialProfile::default(),
        raster: settings,
        plan: windows.plan().clone(),
        volume_mm3: 0.0,
        exposure: ExposurePlan::default(),
        thumbnail: None,
        created_unix_s: 0,
    };
    let mut file = Cursor::new(Vec::new());
    write_to(
        &Writing {
            format,
            name: "orientation",
            job: &job,
            source: &mesh,
            windows: &windows,
            settings: &settings,
            window: 2,
            fold: Measured::new(0),
        },
        &mut file,
        &mut (),
    )
    .expect("the slab writes")
    .expect("nothing cancelled the run");
    file.into_inner()
}

/// The first and last row of the first layer of `bytes`, a file called `name`, that hold
/// a lit pixel.
fn lit_rows_of(bytes: Vec<u8>, name: &str) -> (u32, u32) {
    let mut opened = open(Path::new(name), Cursor::new(bytes)).expect("the file reads back");
    let mut lit = Vec::new();
    let mut at = 0;
    for run in opened.layer(0).expect("the first layer") {
        if run.value > 0 {
            lit.extend([at / WIDTH_PX, (at + run.length - 1) / WIDTH_PX]);
        }
        at += run.length;
    }
    let first = *lit.iter().min().expect("the slab lights pixels");
    let last = *lit.iter().max().expect("the slab lights pixels");
    (first, last)
}

#[test]
fn an_unmirrored_machine_gets_rows_from_the_far_edge() {
    let rows = lit_rows(printer(false), SlicedFormat::Goo, "normal.goo");
    assert_eq!(
        rows,
        (HEIGHT_PX - 8, HEIGHT_PX - 3),
        "a slab 0.4 to 1.6 mm from the near edge sits in the last rows"
    );
}

#[test]
fn an_x_mirrored_machine_gets_the_plate_as_it_stands() {
    let rows = lit_rows(printer(true), SlicedFormat::Goo, "mirrored.goo");
    assert_eq!(
        rows,
        (2, 7),
        "the slab sits in rows 2 to 7 from the near edge"
    );
}

#[test]
fn photons_starts_at_the_far_edge_whatever_the_mirror() {
    let rows = lit_rows(printer(true), SlicedFormat::Photons, "mirrored.photons");
    assert_eq!(
        rows,
        (HEIGHT_PX - 8, HEIGHT_PX - 3),
        "the format mirrors nothing itself"
    );
}

#[test]
fn cxline_keeps_the_plate_as_it_stands() {
    let rows = lit_rows(printer(false), SlicedFormat::Cxline, "normal.cxline");
    assert_eq!(rows, (2, 7), "the vendor's .cxline matched without a flip");
}

#[test]
fn a_conversion_lays_the_rows_as_the_new_container_does() {
    let printer = printer(true);
    let goo = written(printer.clone(), SlicedFormat::Goo);
    let mut source = open(Path::new("mirrored.goo"), Cursor::new(goo)).expect("reads back");
    let resin = MaterialProfile::default();
    let mut converted = Cursor::new(Vec::new());
    convert_to(
        &mut source,
        &Converting {
            format: SlicedFormat::Photons,
            name: "orientation",
            printer: &printer,
            material: &resin,
            created_unix_s: 0,
        },
        &mut converted,
        &mut (),
    )
    .expect("the file converts")
    .expect("nothing cancelled the run");
    assert_eq!(
        lit_rows_of(converted.into_inner(), "mirrored.photons"),
        (HEIGHT_PX - 8, HEIGHT_PX - 3),
        "the slab moves to the far edge, as a .photons cut for this machine has it"
    );
}

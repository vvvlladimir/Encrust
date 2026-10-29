//! The stage both front ends share: a cut stack written into a printable file.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::num::NonZeroU8;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use core_analysis::Measured;
use core_format::{ExposurePlan, PrintJob};
use core_geometry::{Mesh, Vec3};
use core_pipeline::{
    Observer, PanelOverrides, SlicedFormat, Tolerance, Writing, raster_settings,
    write as write_stack,
};
use core_raster::{GreyRange, RasterSettings, Shading};
use core_slicer::{ONE_SAMPLE, SliceSettings, Windows};
use format_chitu::CtbVersion;
use printer_profiles::{MaterialProfile, PrinterProfile};

/// Panel of 64 x 32 pixels at a 0.2 mm pitch: 12.8 x 6.4 mm of build area.
const PROFILE: &str = r#"
name = "Test panel"
manufacturer = "Test"

[display]
width_px = 64
height_px = 32
width_mm = 12.8
height_mm = 6.4

[build_volume]
x = 12.8
y = 6.4
z = 10.0
"#;

/// A closed box, twelve triangles, wound outward.
fn box_mesh(mins: Vec3, maxs: Vec3) -> Mesh {
    core_geometry::cuboid(mins, maxs)
}

/// 4 x 4 x 2 mm, standing on the plate well inside the panel.
fn on_the_plate() -> Mesh {
    box_mesh(Vec3::new(1.0, 1.0, 0.0), Vec3::new(5.0, 5.0, 2.0))
}

fn printer() -> PrinterProfile {
    PrinterProfile::from_toml_str(PROFILE, Path::new("inline.toml"))
        .expect("the inline profile is valid")
}

fn panel() -> RasterSettings {
    raster_settings(
        &printer(),
        PanelOverrides {
            shading: Shading::Coverage,
            grey_levels: None,
            grey_range: None,
            blur_px: 0,
        },
    )
}

fn windows_of(mesh: &Mesh, window: usize) -> Windows {
    Windows::new(
        mesh,
        SliceSettings {
            layer_height: 0.5,
            samples: ONE_SAMPLE,
        },
        window,
    )
    .expect("a closed box slices")
}

fn job(plan: core_slicer::LayerPlan, raster: RasterSettings) -> PrintJob {
    PrintJob {
        printer: printer(),
        material: MaterialProfile::default(),
        raster,
        plan,
        volume_mm3: 0.0,
        exposure: ExposurePlan::default(),
        thumbnail: None,
        created_unix_s: 0,
    }
}

/// A path of its own per test, since the pipeline writes a real file.
fn temp(name: &str, extension: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "encrust-pipeline-{}-{name}.{extension}",
        std::process::id()
    ))
}

/// What a file on disk is called, without its directory or its extension.
fn name_of(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Writes `mesh` and hands back the file's path and what the run came to.
fn run(
    mesh: &Mesh,
    path: &Path,
    format: SlicedFormat,
    window: usize,
    fold: Measured,
    observer: &mut dyn Observer,
) -> Option<core_pipeline::Written> {
    let raster = panel();
    let windows = windows_of(mesh, 1024);
    let job = job(windows.plan().clone(), raster);
    write_stack(
        &Writing {
            format,
            name: &name_of(path),
            job: &job,
            source: mesh,
            windows: &windows,
            settings: &raster,
            window,
            fold,
        },
        path,
        observer,
    )
    .expect("the box writes")
}

#[test]
fn the_grey_range_is_the_profiles_unless_one_is_asked_for() {
    let mut printer = printer();
    (printer.display.grey_min, printer.display.grey_max) = (127, 255);
    let range = |asked| {
        let overrides = PanelOverrides {
            grey_range: asked,
            ..PanelOverrides::default()
        };
        raster_settings(&printer, overrides).grey.range
    };
    assert_eq!(range(None), GreyRange { min: 127, max: 255 });
    let asked = GreyRange { min: 0, max: 200 };
    assert_eq!(range(Some(asked)), asked);
}

#[test]
fn a_binary_only_panel_gets_no_grey_whatever_was_asked() {
    let mut printer = printer();
    printer.display.binary_only = true;
    let settings = raster_settings(
        &printer,
        PanelOverrides {
            shading: Shading::Coverage,
            ..PanelOverrides::default()
        },
    );
    assert_eq!(settings.shading, Shading::Binary);
}

#[test]
fn a_box_reaches_the_file_as_the_layers_it_cures() {
    let path = temp("box", "goo");
    let written = run(
        &on_the_plate(),
        &path,
        SlicedFormat::Goo,
        2,
        Measured::new(0),
        &mut (),
    )
    .expect("nothing cancelled the run");

    // 2 mm of model at half a millimetre a layer, 4 x 4 mm of footprint.
    assert_eq!(written.layers, 4);
    assert_eq!(
        written.clipped_layers, 0,
        "the box is well inside the panel"
    );
    assert!(
        (written.measured.volume_mm3() - 32.0).abs() < 0.5,
        "expected about 32 mm^3, got {}",
        written.measured.volume_mm3()
    );
    assert!(
        std::fs::metadata(&path).expect("the file is on disk").len() > 0,
        "the run wrote bytes"
    );
    std::fs::remove_file(&path).expect("the test wrote the file");
}

#[test]
fn the_extension_chooses_the_writer_from_the_same_request() {
    let path = temp("revision", "ctb");
    run(
        &on_the_plate(),
        &path,
        SlicedFormat::Ctb(CtbVersion::V5),
        2,
        Measured::new(0),
        &mut (),
    )
    .expect("nothing cancelled the run");

    let file = std::fs::read(&path).expect("the file is on disk");
    assert_eq!(
        &file[..4],
        &0x12FD_0106u32.to_le_bytes(),
        "a .ctb name has to reach the ctb writer, not the goo one"
    );
    assert_eq!(
        u32::from_le_bytes([file[4], file[5], file[6], file[7]]),
        5,
        "the request asked for version 5"
    );
    std::fs::remove_file(&path).expect("the test wrote the file");
}

#[test]
fn the_window_size_does_not_change_the_file() {
    // The stack is cut and rasterised a window at a time, so a run with a different
    // window has to produce the same bytes; see ADR 0068.
    let mesh = on_the_plate();
    let wide = temp("wide-window", "goo");
    let narrow = temp("narrow-window", "goo");
    run(
        &mesh,
        &wide,
        SlicedFormat::Goo,
        64,
        Measured::new(0),
        &mut (),
    );
    run(
        &mesh,
        &narrow,
        SlicedFormat::Goo,
        1,
        Measured::new(0),
        &mut (),
    );

    let one = std::fs::read(&wide).expect("the file is written");
    let other = std::fs::read(&narrow).expect("the file is written");
    let _ = std::fs::remove_file(&wide);
    let _ = std::fs::remove_file(&narrow);

    assert_eq!(one.len(), other.len());
    // The header carries the time the file was written, which is the one field two runs
    // a moment apart may disagree on.
    assert_eq!(one[195_000..], other[195_000..]);
}

#[test]
fn an_island_is_taken_out_of_the_file_when_the_fold_is_asked_to() {
    // The box on the plate, and a 2 x 2 mm one beside it standing on nothing from 1 mm up.
    let mut mesh = on_the_plate();
    let floating = box_mesh(Vec3::new(7.0, 1.0, 1.0), Vec3::new(9.0, 3.0, 2.0));
    let base = mesh.vertices.len() as u32;
    mesh.vertices.extend(&floating.vertices);
    mesh.faces.extend(
        floating
            .faces
            .iter()
            .map(|face| face.map(|index| index + base)),
    );

    let volume = |fold: Measured, name: &str| {
        let path = temp(name, "goo");
        let written = run(&mesh, &path, SlicedFormat::Goo, 2, fold, &mut ())
            .expect("nothing cancelled the run");
        let _ = std::fs::remove_file(&path);
        written.measured.volume_mm3()
    };

    // 32 mm^3 of the box on the plate, and 4 mm^3 of the floating one; removing takes its
    // first 0.5 mm layer alone, 2 mm^3, and leaves what stood on it.
    assert!((volume(Measured::new(0), "islands-kept") - 36.0).abs() < 0.5);
    assert!((volume(Measured::new(0).removing_islands(), "islands-removed") - 34.0).abs() < 0.5);
}

/// Stops the run as soon as the first group of layers has been written.
struct StopAfterOne {
    seen: AtomicBool,
    layers: usize,
}

impl Observer for StopAfterOne {
    fn layers(&mut self, done: usize, total: usize) {
        assert_eq!(total, 4, "a 2 mm box at half a millimetre a layer");
        self.layers = done;
        self.seen.store(true, Ordering::Relaxed);
    }

    fn cancelled(&self) -> bool {
        self.seen.load(Ordering::Relaxed)
    }
}

#[test]
fn a_cancelled_run_leaves_no_half_written_file_behind() {
    let path = temp("cancelled", "goo");
    let mut observer = StopAfterOne {
        seen: AtomicBool::new(false),
        layers: 0,
    };
    let written = run(
        &on_the_plate(),
        &path,
        SlicedFormat::Goo,
        1,
        Measured::new(0),
        &mut observer,
    );

    assert!(written.is_none(), "a cancelled run reports nothing written");
    assert_eq!(
        observer.layers, 1,
        "the cancel landed after the first layer"
    );
    assert!(
        !path.exists(),
        "a file stopped part way would still look printable"
    );
}

#[test]
fn a_run_whose_file_cannot_be_created_names_the_path() {
    let path = temp("missing", "goo").join("nested").join("cube.goo");
    let raster = panel();
    let mesh = on_the_plate();
    let windows = windows_of(&mesh, 1024);
    let job = job(windows.plan().clone(), raster);

    let error = write_stack(
        &Writing {
            format: SlicedFormat::Goo,
            name: &name_of(&path),
            job: &job,
            source: &mesh,
            windows: &windows,
            settings: &raster,
            window: 2,
            fold: Measured::new(0),
        },
        &path,
        &mut (),
    )
    .expect_err("writing into a directory that does not exist cannot succeed");

    assert!(
        matches!(&error, core_pipeline::PipelineError::Create { path: named, .. } if named == &path),
        "got {error:?}"
    );
}

#[test]
fn a_panel_that_cannot_be_drawn_names_the_layer_it_failed_on() {
    let mesh = on_the_plate();
    let windows = windows_of(&mesh, 1024);
    let raster = RasterSettings {
        width_px: 0,
        ..panel()
    };

    let error = core_pipeline::measure(
        &mesh,
        &windows,
        &raster,
        &Tolerance::default(),
        Measured::new(0),
        &mut (),
    )
    .expect_err("a panel with no pixels draws no mask");

    // Layers are rasterised in parallel, so whichever fails first is the one named; each
    // plane sits mid-layer, see docs/design/slicing.md
    let core_pipeline::PipelineError::Raster { z, .. } = error else {
        panic!("got {error:?}");
    };
    let layer = (z - 0.25) / 0.5;
    assert!(
        (layer - layer.round()).abs() < 1e-4 && (0.0..4.0).contains(&layer),
        "z = {z} mm is not the plane of any of the box's four layers"
    );
}

#[test]
fn a_bottom_block_of_layers_is_held_at_the_fold() {
    let path = temp("bottom-block", "goo");
    let written = run(
        &on_the_plate(),
        &path,
        SlicedFormat::Goo,
        2,
        Measured::new(2),
        &mut (),
    )
    .expect("nothing cancelled the run");
    let _ = std::fs::remove_file(&path);

    assert_eq!(written.layers, 4);
    assert_eq!(
        written.measured.layer_count(),
        4,
        "every layer is folded, bottom block included"
    );
}

#[test]
fn a_panel_the_model_overhangs_is_counted_as_clipped() {
    // 12 mm wide on a 12.8 mm panel, but starting 6 mm in, so it runs off the right edge.
    let mesh = box_mesh(Vec3::new(6.0, 1.0, 0.0), Vec3::new(18.0, 5.0, 2.0));
    let path = temp("clipped", "goo");
    let written = run(
        &mesh,
        &path,
        SlicedFormat::Goo,
        2,
        Measured::new(0),
        &mut (),
    )
    .expect("nothing cancelled the run");
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        written.clipped_layers, 4,
        "every layer reaches past the panel"
    );
    assert!(written.max_overflow_px > 0.0);
}

/// `NonZeroU8` is in the public surface through `PanelOverrides`; a run that rounds its
/// greys still has to produce a file.
#[test]
fn rounding_the_greys_still_writes_the_stack() {
    let raster = raster_settings(
        &printer(),
        PanelOverrides {
            shading: Shading::Coverage,
            grey_levels: NonZeroU8::new(8),
            grey_range: Some(GreyRange { min: 16, max: 240 }),
            blur_px: 1,
        },
    );
    let mesh = on_the_plate();
    let windows = windows_of(&mesh, 1024);
    let job = job(windows.plan().clone(), raster);
    let path = temp("greys", "goo");

    let written = write_stack(
        &Writing {
            format: SlicedFormat::Goo,
            name: &name_of(&path),
            job: &job,
            source: &mesh,
            windows: &windows,
            settings: &raster,
            window: 2,
            fold: Measured::new(0),
        },
        &path,
        &mut (),
    )
    .expect("the box writes")
    .expect("nothing cancelled the run");
    let _ = std::fs::remove_file(&path);

    assert_eq!(written.layers, 4);
}

/// A diamond floating beside the box: an octahedron of radius 2.4 mm, its lower point
/// 1.1 mm over the plate, so its first layers grow out of a point over nothing.
fn with_a_floating_diamond() -> Mesh {
    let mut mesh = on_the_plate();
    let centre = Vec3::new(9.6, 3.2, 3.5);
    let r = 2.4;
    let corners = [
        centre + Vec3::X * r,
        centre - Vec3::X * r,
        centre + Vec3::Y * r,
        centre - Vec3::Y * r,
        centre + Vec3::Z * r,
        centre - Vec3::Z * r,
    ];
    let base = mesh.vertices.len() as u32;
    mesh.vertices.extend(corners);
    let faces: [[u32; 3]; 8] = [
        [0, 2, 4],
        [2, 1, 4],
        [1, 3, 4],
        [3, 0, 4],
        [2, 0, 5],
        [1, 2, 5],
        [3, 1, 5],
        [0, 3, 5],
    ];
    mesh.faces
        .extend(faces.iter().map(|face| face.map(|index| index + base)));
    mesh
}

#[test]
fn removing_islands_takes_the_point_of_a_floating_diamond_and_leaves_the_rest() {
    let mesh = with_a_floating_diamond();
    let fold = |fold: Measured, name: &str| {
        let path = temp(name, "goo");
        let written = run(&mesh, &path, SlicedFormat::Goo, 2, fold, &mut ())
            .expect("nothing cancelled the run");
        let _ = std::fs::remove_file(&path);
        written.measured
    };
    let kept = fold(Measured::new(0), "diamond-kept");
    let removed = fold(Measured::new(0).removing_islands(), "diamond-islands");
    let whole = fold(
        Measured::new(0).removing_hanging(u32::MAX),
        "diamond-hanging",
    );
    assert_eq!(removed.removed_layers(), 1, "the point's layer alone");
    assert!(
        kept.volume_mm3() - removed.volume_mm3() < 0.25 * (kept.volume_mm3() - whole.volume_mm3()),
        "most of the diamond stays"
    );
    assert!(
        (whole.volume_mm3() - 32.0).abs() < 0.5,
        "passes enough to reach its top take the whole diamond, leaving the 32 mm^3 box"
    );
}

//! A plate of placed models into a sliced file, through the public entry points alone.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use core_engine::{Cutting, Model, Plate, Run, bake, cut, parts};
use core_format::ExposurePlan;
use core_geometry::{Aabb, Mesh, Scalar, Transform, Vec3};
use core_pipeline::{Observer, PanelOverrides, SlicedFormat};
use core_slicer::Sliced;
use format_chitu::CtbVersion;
use printer_profiles::{Compensation, MaterialProfile, PrinterProfile};

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
    let vertices = vec![
        Vec3::new(mins.x, mins.y, mins.z),
        Vec3::new(maxs.x, mins.y, mins.z),
        Vec3::new(maxs.x, maxs.y, mins.z),
        Vec3::new(mins.x, maxs.y, mins.z),
        Vec3::new(mins.x, mins.y, maxs.z),
        Vec3::new(maxs.x, mins.y, maxs.z),
        Vec3::new(maxs.x, maxs.y, maxs.z),
        Vec3::new(mins.x, maxs.y, maxs.z),
    ];
    let faces = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    Mesh::new(vertices, faces)
}

/// The unit cube, spanning 0..1 on every axis.
fn unit_cube() -> Arc<Mesh> {
    Arc::new(box_mesh(Vec3::ZERO, Vec3::ONE))
}

fn cube_at(x: Scalar) -> Model {
    Model::placed(
        unit_cube(),
        Transform::from_translation(Vec3::new(x, 0.0, 0.0)),
    )
}

/// A plate holding one 4 x 4 x 2 mm box, standing well inside the panel.
fn plate(format: SlicedFormat) -> Plate {
    let mesh = Arc::new(box_mesh(Vec3::new(1.0, 1.0, 0.0), Vec3::new(5.0, 5.0, 2.0)));
    Plate {
        models: vec![Model::placed(mesh, Transform::default())],
        printer: PrinterProfile::from_toml_str(PROFILE, Path::new("inline.toml"))
            .expect("the inline profile is valid"),
        material: MaterialProfile::default(),
        panel: PanelOverrides::default(),
        cutting: Cutting::uniform(0.5),
        exposure: ExposurePlan::default(),
        remove_islands: false,
        format,
        raster_window: 2,
    }
}

#[test]
fn an_empty_plate_has_nothing_to_bake() {
    assert!(bake(&[], &Compensation::default()).is_none());
}

#[test]
fn two_models_keep_all_their_faces_and_their_places() {
    let merged = bake(&[cube_at(0.0), cube_at(5.0)], &Compensation::default())
        .expect("two cubes have geometry");

    assert_eq!(merged.faces.len(), 24);
    assert_eq!(merged.vertices.len(), 16);
    let bounds = merged.aabb().expect("the bake has vertices");
    assert!(bounds.mins.abs_diff_eq(Vec3::ZERO, 1e-5));
    assert!(bounds.maxs.abs_diff_eq(Vec3::new(6.0, 1.0, 1.0), 1e-5));
}

#[test]
fn the_second_models_faces_point_at_its_own_vertices() {
    let merged = bake(&[cube_at(0.0), cube_at(5.0)], &Compensation::default())
        .expect("two cubes have geometry");

    for face in &merged.faces[12..] {
        assert!(
            face.iter().all(|index| *index >= 8),
            "the second cube must index its own vertices"
        );
    }
}

#[test]
fn a_support_goes_in_where_it_already_stands() {
    // A cube ten millimetres across, hanging ten millimetres up, with a pillar standing
    // under it in plate coordinates.
    let model = Model {
        mesh: unit_cube(),
        transform: Transform {
            translation: Vec3::new(0.0, 0.0, 10.0),
            scale: Vec3::splat(10.0),
            ..Transform::default()
        },
        cuts: None,
        supports: vec![Arc::new(box_mesh(
            Vec3::new(4.0, 4.0, 0.0),
            Vec3::new(6.0, 6.0, 10.0),
        ))],
    };

    let merged = bake(std::slice::from_ref(&model), &Compensation::default())
        .expect("the plate has geometry");
    assert_eq!(merged.faces.len(), 24, "the cube and the pillar");
    let bounds = merged.aabb().expect("the bake has vertices");
    assert!(
        bounds.mins.z <= 0.0,
        "the pillar reaches the plate, so the bake starts at z = 0, got {}",
        bounds.mins.z
    );
}

#[test]
fn a_shrinking_resin_prints_the_part_larger_where_it_already_stood() {
    let models = [cube_at(30.0)];
    let plain = bake(&models, &Compensation::default()).expect("a cube is there");
    let grown = bake(
        &models,
        &Compensation {
            shrink_x_pct: 101.0,
            shrink_y_pct: 101.0,
            ..Compensation::default()
        },
    )
    .expect("a cube is there");

    let before = plain.aabb().expect("a cube has bounds");
    let after = grown.aabb().expect("a cube has bounds");
    let width = |bounds: &Aabb| bounds.maxs.x - bounds.mins.x;
    let middle = |bounds: &Aabb| Scalar::midpoint(bounds.mins.x, bounds.maxs.x);
    assert!(
        (width(&after) / width(&before) - 1.01).abs() < 1e-4,
        "a unit cube comes out one percent wider"
    );
    assert!(
        (middle(&after) - middle(&before)).abs() < 1e-4,
        "and still stands where it stood"
    );
    assert!(
        (after.mins.z - before.mins.z).abs() < 1e-4,
        "and still sits on the plate"
    );
}

#[test]
fn a_models_supports_are_listed_beside_it_without_being_copied() {
    let supports = Arc::new(box_mesh(Vec3::ZERO, Vec3::splat(2.0)));
    let model = Model {
        supports: vec![Arc::clone(&supports)],
        ..cube_at(3.0)
    };

    let listed = parts(std::slice::from_ref(&model));
    assert_eq!(listed.len(), 2);
    assert_eq!(
        listed[0].1, model.transform,
        "the model keeps its placement"
    );
    assert_eq!(
        listed[1].1,
        Transform::default(),
        "supports are already in plate coordinates"
    );
    assert!(Arc::ptr_eq(&listed[1].0, &supports), "nothing is copied");
}

#[test]
fn every_layer_comes_through_exactly_once_whatever_the_window() {
    let mesh = box_mesh(Vec3::ZERO, Vec3::ONE);
    let collect = |slice_window: usize| {
        let windows = cut(
            &mesh,
            &Cutting {
                slice_window,
                ..Cutting::uniform(0.1)
            },
        )
        .expect("a cube slices");
        let mut heights = Vec::new();
        let mut widest = 0;
        windows
            .stream(&mesh, |sliced: &Sliced| {
                widest = widest.max(sliced.layers.len());
                heights.extend(sliced.layers.iter().map(|layer| layer.z));
                Ok::<(), core_slicer::SliceError>(())
            })
            .expect("a cube slices");
        (heights, widest)
    };

    let (whole, _) = collect(1024);
    let (chopped, widest) = collect(3);
    assert_eq!(whole.len(), 10, "a 1 mm cube at 0.1 mm is ten layers");
    assert_eq!(whole, chopped);
    assert_eq!(widest, 3, "a window holds at most its own layers");
}

/// Every window and every layer count the run reported, in the order it reported them.
#[derive(Default)]
struct Watching {
    windows: usize,
    layers: Vec<usize>,
}

impl Observer for Watching {
    fn window(&mut self, _sliced: &Sliced) {
        self.windows += 1;
    }

    fn layers(&mut self, done: usize, total: usize) {
        assert_eq!(total, 4, "2 mm of model at half a millimetre a layer");
        self.layers.push(done);
    }
}

#[test]
fn a_run_writes_every_layer_into_the_sink_and_reports_them_in_order() {
    let run = Run::of(&plate(SlicedFormat::Goo)).expect("a box on the panel is a run");
    assert_eq!(run.layer_count(), 4);

    let mut file = Cursor::new(Vec::new());
    let mut watching = Watching::default();
    let written = run
        .write("cube", &mut file, &mut watching)
        .expect("the run writes")
        .expect("nothing cancelled it");

    assert_eq!(written.layers, 4);
    assert_eq!(
        written.clipped_layers, 0,
        "the box is well inside the panel"
    );
    // 4 x 4 mm of footprint over 2 mm of height.
    assert!(
        (written.measured.volume_mm3() - 32.0).abs() < 0.5,
        "expected about 32 mm^3, got {}",
        written.measured.volume_mm3()
    );
    assert_eq!(watching.layers, vec![2, 4], "two layers to a raster window");
    assert!(watching.windows >= 1);
    assert!(!file.into_inner().is_empty());
}

#[test]
fn an_empty_plate_is_not_a_run() {
    let plate = Plate {
        models: Vec::new(),
        ..plate(SlicedFormat::Goo)
    };
    assert!(matches!(
        Run::of(&plate),
        Err(core_engine::EngineError::EmptyPlate)
    ));
}

#[test]
fn the_requested_revision_reaches_the_ctb_header() {
    let run = Run::of(&plate(SlicedFormat::Ctb(CtbVersion::V5))).expect("the box is a run");
    let mut file = Cursor::new(Vec::new());
    run.write("cube", &mut file, &mut ())
        .expect("the run writes")
        .expect("nothing cancelled it");

    let bytes = file.into_inner();
    assert_eq!(
        u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        5
    );
}

#[test]
fn the_written_file_carries_a_picture_of_the_plate() {
    let run = Run::of(&plate(SlicedFormat::Goo)).expect("the box is a run");
    let mut file = Cursor::new(Vec::new());
    run.write("cube", &mut file, &mut ())
        .expect("the run writes")
        .expect("nothing cancelled it");

    // The small preview is 116 by 116 RGB565 at a fixed offset; see docs/formats/goo.md
    let bytes = file.into_inner();
    let preview = &bytes[194..194 + 2 * 116 * 116];
    assert!(
        preview.iter().any(|byte| *byte != 0),
        "the box on the plate reaches the preview record"
    );
}

#[test]
fn measuring_a_run_cures_what_writing_it_does() {
    let run = Run::of(&plate(SlicedFormat::Goo)).expect("the box is a run");
    let measured = run
        .measure(&mut ())
        .expect("the stack measures")
        .expect("nothing cancelled it");

    let mut file = Cursor::new(Vec::new());
    let written = run
        .write("cube", &mut file, &mut ())
        .expect("the run writes")
        .expect("nothing cancelled it");

    assert_eq!(measured.layer_count(), written.measured.layer_count());
    assert!(
        (measured.volume_mm3() - written.measured.volume_mm3()).abs() < 1e-3,
        "measuring and writing fold the same stack"
    );
}

/// An observer that stops the run as soon as the first group of layers has landed.
struct StopAtOnce(std::cell::Cell<bool>);

impl Observer for StopAtOnce {
    fn layers(&mut self, _done: usize, _total: usize) {
        self.0.set(true);
    }

    fn cancelled(&self) -> bool {
        self.0.get()
    }
}

#[test]
fn a_cancelled_run_comes_back_as_nothing_written() {
    let run = Run::of(&plate(SlicedFormat::Goo)).expect("the box is a run");
    let mut file = Cursor::new(Vec::new());
    let written = run
        .write(
            "cube",
            &mut file,
            &mut StopAtOnce(std::cell::Cell::new(false)),
        )
        .expect("stopping a run is not a failure");
    assert!(written.is_none());
}

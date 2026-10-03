//! A `.encrust` project opened into a plate, with what the file leaves out built again.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::sync::Arc;

use core_engine::project::{
    Array, Axis, Cavity, Chosen, CutState, DrainState, Group, HollowState, Keep, Manifest,
    ObjectHollowState, ObjectState, ObjectSupportState, Project, SlicingState, Summary,
    SupportState, VERSION,
};
use core_engine::{EngineError, Opening, open_plate};
use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose, signed_volume};
use core_supports::{ProjectSettings, SupportPoint};
use core_volume::{HollowMode, InfillSettings, VolumeError};
use printer_profiles::{MaterialProfile, OutputFormat, PrinterProfile, SupportProfile};

const PRINTER: &str = r#"
name = "Test panel"
manufacturer = "Test"

[display]
width_px = 400
height_px = 400
width_mm = 80.0
height_mm = 80.0

[build_volume]
x = 80.0
y = 80.0
z = 80.0
"#;

/// A closed box, twelve triangles, wound outward.
fn box_mesh(mins: Vec3, maxs: Vec3) -> Mesh {
    let corner = |x: bool, y: bool, z: bool| {
        Vec3::new(
            if x { maxs.x } else { mins.x },
            if y { maxs.y } else { mins.y },
            if z { maxs.z } else { mins.z },
        )
    };
    let vertices = vec![
        corner(false, false, false),
        corner(true, false, false),
        corner(true, true, false),
        corner(false, true, false),
        corner(false, false, true),
        corner(true, false, true),
        corner(true, true, true),
        corner(false, true, true),
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

fn object(name: &str, mesh: &Mesh, transform: Transform) -> ObjectState {
    ObjectState {
        name: name.to_owned(),
        plate: 0,
        transform,
        visible: true,
        summary: Summary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(mesh),
        },
        supports: ObjectSupportState::default(),
        hollow: ObjectHollowState::default(),
    }
}

/// A project of one 20 mm cube standing on the plate, on a printer with a resin.
fn project() -> Project {
    let cube = box_mesh(Vec3::ZERO, Vec3::splat(20.0));
    let printer = PrinterProfile::from_toml_str(PRINTER, std::path::Path::new("inline.toml"))
        .expect("the inline profile is valid");
    Project {
        manifest: Manifest {
            version: VERSION,
            printer: Some(Chosen {
                id: None,
                profile: printer,
            }),
            resin: Some(Chosen {
                id: None,
                profile: MaterialProfile::default(),
            }),
            slicing: SlicingState {
                layer_height_mm: 0.05,
                adaptive: None,
                exposure: Vec::new(),
                samples: core_slicer::ONE_SAMPLE,
                anti_alias: true,
                grey_levels: None,
                blur_px: 0,
                remove_islands: false,
                format: OutputFormat::Goo,
            },
            supports: SupportState {
                groups: vec![Group {
                    name: "Default".to_owned(),
                    profile: SupportProfile::medium(),
                }],
                active: 0,
                fill: ProjectSettings::default(),
                brush_radius_mm: 2.0,
                flood_angle_deg: 30.0,
            },
            hollow: HollowState {
                thickness_mm: 2.0,
                mode: HollowMode::default(),
                precision: 0.5,
                infill_on: false,
                infill: InfillSettings::default(),
                blocker_mm: 4.0,
            },
            drain: DrainState {
                diameter_mm: 3.0,
                depth_mm: 3.0,
                taper: 1.0,
            },
            cut: CutState {
                axis: Axis::Z,
                height_mm: 10.0,
                keep: Keep::Both,
            },
            array: Array::default(),
            plates: vec!["Plate 1".to_owned()],
            active_plate: 0,
            objects: vec![object("cube", &cube, Transform::default())],
        },
        meshes: vec![Arc::new(cube)],
    }
}

fn opening() -> Opening {
    Opening {
        plate: 0,
        hollow_budget_bytes: 256 << 20,
        raster_window: 2,
        created_unix_s: 0,
    }
}

fn wall(thickness_mm: f32) -> Cavity {
    Cavity {
        thickness_mm,
        mode: HollowMode::default(),
        precision: 0.5,
        infill: None,
    }
}

#[test]
fn a_solid_model_opens_as_the_mesh_it_was_saved_with() {
    let saved = project();
    let plate = open_plate(&saved, &opening()).expect("the project is complete");

    let [model] = plate.models.as_slice() else {
        panic!("one object on the plate");
    };
    assert!(
        Arc::ptr_eq(&model.mesh, &saved.meshes[0]),
        "nothing to rebuild"
    );
    assert!(model.cuts.is_none(), "no hole was drilled");
    assert!(
        model.supports.iter().all(|group| group.faces.is_empty()),
        "no support was placed"
    );
}

#[test]
fn a_hollowed_model_opens_as_its_shell() {
    let mut saved = project();
    saved.manifest.objects[0].hollow.cavity = Some(wall(2.0));
    let plate = open_plate(&saved, &opening()).expect("the project is complete");

    // A 20 mm cube with a 2 mm wall keeps 20^3 - 16^3 mm3; the cavity comes off a lattice
    // a fraction of a millimetre across, so its faces sit within a voxel of the plane.
    let expected = 20.0f32.powi(3) - 16.0f32.powi(3);
    let kept = signed_volume(&plate.models[0].mesh);
    assert!(
        (kept - expected).abs() / expected < 0.05,
        "expected about {expected} mm3 of wall, got {kept}"
    );
}

#[test]
fn a_cavity_that_cannot_be_built_names_its_model() {
    let mut saved = project();
    saved.manifest.objects[0].hollow.cavity = Some(wall(0.0));

    let error = open_plate(&saved, &opening()).expect_err("a wall of no thickness");
    assert!(matches!(
        error,
        EngineError::Hollow { ref object, source: VolumeError::BadThickness(_) } if object == "cube"
    ));
}

#[test]
fn a_support_point_grows_into_a_column_down_to_the_plate() {
    let mut saved = project();
    let lifted = Transform::from_translation(Vec3::new(30.0, 30.0, 10.0));
    let object = &mut saved.manifest.objects[0];
    object.transform = lifted;
    object.supports.points = vec![SupportPoint::new(Vec3::new(10.0, 10.0, 0.0))];

    let plate = open_plate(&saved, &opening()).expect("the project is complete");
    let [column] = plate.models[0].supports.as_slice() else {
        panic!("one group of supports");
    };
    let bounds = column.aabb().expect("the column has geometry");
    assert!(
        bounds.mins.z <= 0.0,
        "it stands on the plate, got {}",
        bounds.mins.z
    );
}

#[test]
fn only_what_is_visible_on_the_opened_plate_is_taken() {
    let mut saved = project();
    let cube = Arc::clone(&saved.meshes[0]);
    let mut elsewhere = object("elsewhere", &cube, Transform::default());
    elsewhere.plate = 1;
    let mut hidden = object("hidden", &cube, Transform::default());
    hidden.visible = false;
    saved.manifest.objects.extend([elsewhere, hidden]);
    saved.meshes.extend([Arc::clone(&cube), cube]);

    let plate = open_plate(&saved, &opening()).expect("the project is complete");
    assert_eq!(plate.models.len(), 1);
}

#[test]
fn a_project_without_a_printer_is_refused() {
    let mut saved = project();
    saved.manifest.printer = None;
    let error = open_plate(&saved, &opening()).expect_err("there is no panel to cut for");
    assert!(matches!(error, EngineError::NoPrinter));
}

#[test]
fn a_project_without_a_resin_is_refused() {
    let mut saved = project();
    saved.manifest.resin = None;
    let error = open_plate(&saved, &opening()).expect_err("there is nothing to expose");
    assert!(matches!(error, EngineError::NoResin));
}

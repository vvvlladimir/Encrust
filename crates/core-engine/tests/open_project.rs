//! A `.encrust` project opened into a plate, which is the file's own geometry: nothing
//! is hollowed again and no support is grown again.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::sync::Arc;

use core_engine::project::{
    Array, Axis, BuiltCavity, Cavity, Chosen, CutState, DrainState, Group, HollowState, Keep,
    Manifest, ModelMeshes, ObjectHollowState, ObjectState, ObjectSupportState, Project,
    SlicingState, Summary, SupportState, VERSION,
};
use core_engine::{EngineError, Opening, open_plate};
use core_geometry::{Bvh, Mesh, Orientation, Transform, Vec3, diagnose};
use core_supports::{ModelSupports, ProjectSettings, SupportPoint, SupportTree};
use core_volume::{HollowMode, InfillSettings};
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
        models: vec![ModelMeshes {
            source: Arc::new(cube),
            shell: None,
        }],
    }
}

fn opening() -> Opening {
    Opening {
        plate: 0,
        raster_window: 2,
        created_unix_s: 0,
    }
}

/// A shell of `thickness_mm` as a run would have left it, standing on `saved`'s only
/// model: a box 2 mm inside the cube, which no run would produce from the cube itself.
fn hollowed(saved: &mut Project, thickness_mm: f32) {
    saved.manifest.objects[0].hollow.built = Some(BuiltCavity {
        wall: Cavity {
            thickness_mm,
            mode: HollowMode::default(),
            precision: 0.5,
            infill: None,
        },
        cavity_faces: 0..12,
        cavity_mm3: 16.0f32.powi(3),
        voxel_mm: 0.2,
        coarsened: false,
        scale: Vec3::ONE,
    });
    saved.models[0].shell = Some(Arc::new(box_mesh(
        Vec3::splat(thickness_mm),
        Vec3::splat(20.0 - thickness_mm),
    )));
}

/// The trees an automatic run grows under a cube standing at `transform` from one point,
/// in the model's own space: what the window writes into a project.
fn grown_under(mesh: &Mesh, transform: Transform, contact: Vec3) -> Vec<SupportTree> {
    let mut supports = ModelSupports::default();
    supports.add(contact, transform, 0);
    supports.refresh(
        mesh,
        &Bvh::build(mesh),
        transform,
        std::slice::from_ref(&SupportProfile::medium()),
    );
    supports.grown(transform)
}

#[test]
fn a_solid_model_opens_as_the_mesh_it_was_saved_with() {
    let saved = project();
    let plate = open_plate(&saved, &opening()).expect("the project is complete");

    let [model] = plate.models.as_slice() else {
        panic!("one object on the plate");
    };
    assert!(
        Arc::ptr_eq(&model.mesh, &saved.models[0].source),
        "nothing to rebuild"
    );
    assert!(model.cuts.is_none(), "no hole was drilled");
    assert!(
        model.supports.iter().all(|group| group.faces.is_empty()),
        "no support was placed"
    );
}

#[test]
fn a_hollowed_model_opens_as_the_shell_in_the_file() {
    let mut saved = project();
    hollowed(&mut saved, 2.0);
    let plate = open_plate(&saved, &opening()).expect("the project is complete");

    let shell = saved.models[0]
        .shell
        .as_ref()
        .expect("the model was given a shell");
    assert!(
        Arc::ptr_eq(&plate.models[0].mesh, shell),
        "the shell is read, not hollowed again"
    );
}

#[test]
fn a_tree_the_file_keeps_stands_down_to_the_plate() {
    let mut saved = project();
    let lifted = Transform::from_translation(Vec3::new(30.0, 30.0, 10.0));
    let object = &mut saved.manifest.objects[0];
    object.transform = lifted;
    // The contact is clicked on the plate: the underside of a cube standing at (30, 30, 10).
    object.supports.grown =
        grown_under(&saved.models[0].source, lifted, Vec3::new(40.0, 40.0, 10.0));
    assert_eq!(object.supports.grown.len(), 1, "one point, one tree");

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
fn a_point_without_a_tree_grows_nothing() {
    let mut saved = project();
    saved.manifest.objects[0].supports.points = vec![SupportPoint::new(Vec3::new(10.0, 10.0, 0.0))];

    let plate = open_plate(&saved, &opening()).expect("the project is complete");
    assert!(
        plate.models[0]
            .supports
            .iter()
            .all(|group| group.faces.is_empty()),
        "opening a project decides nothing about where a support goes"
    );
}

#[test]
fn only_what_is_visible_on_the_opened_plate_is_taken() {
    let mut saved = project();
    let cube = Arc::clone(&saved.models[0].source);
    let mut elsewhere = object("elsewhere", &cube, Transform::default());
    elsewhere.plate = 1;
    let mut hidden = object("hidden", &cube, Transform::default());
    hidden.visible = false;
    saved.manifest.objects.extend([elsewhere, hidden]);
    saved.models.extend([
        ModelMeshes {
            source: Arc::clone(&cube),
            shell: None,
        },
        ModelMeshes {
            source: cube,
            shell: None,
        },
    ]);

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

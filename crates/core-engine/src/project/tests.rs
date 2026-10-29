use std::io::Cursor;

use super::*;
use core_geometry::Vec3;
use core_volume::InfillPattern;

/// A plate of `objects` models, each hollowed, so every part of the format is in it:
/// a manifest, a source mesh and a shell beside it.
fn project(objects: usize) -> Project {
    let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
    Project {
        manifest: Manifest {
            version: VERSION,
            printer: Some("elegoo-saturn-4-ultra".to_owned()),
            resin: None,
            slicing: SlicingState {
                remove_islands: false,
                hanging_passes: 0,
                format: OutputFormat::Ctb5,
            },
            supports: SupportState {
                groups: vec![Group {
                    name: "Default".to_owned(),
                    profile: SupportProfile::default(),
                }],
                active: 0,
                fill: ProjectSettings::default(),
                brush_radius_mm: 3.0,
                flood_angle_deg: 30.0,
                inline_spacing_mm: 4.0,
                fill_profiles: FillProfiles::default(),
                raft: None,
                sizing: None,
            },
            hollow: HollowState {
                thickness_mm: 2.0,
                mode: HollowMode::External,
                precision: 0.5,
                infill_on: true,
                infill: InfillSettings::default(),
                blocker_mm: 2.0,
                open_floor: false,
            },
            drain: DrainState::round(3.0, 3.0, 1.0),
            cut: CutState {
                axis: Axis::Z,
                height_mm: 10.0,
                keep: Keep::Both,
                joints: core_joint::Joints::default(),
                parts: PartSettings::default(),
            },
            array: Array::default(),
            plates: vec!["Plate 1".to_owned()],
            active_plate: 0,
            objects: (0..objects).map(object).collect(),
        },
        models: (0..objects)
            .map(|_| ModelMeshes {
                source: Arc::new(mesh.clone()),
                shell: Some(Arc::new(mesh.clone())),
            })
            .collect(),
        preview: Some(b"a picture of the plate".to_vec()),
    }
}

/// One model on the plate, with a point, painted faces, a blocker, a channel and a
/// built cavity, so every part of an object's state is written and read back.
fn object(index: usize) -> ObjectState {
    ObjectState {
        name: format!("model {index}"),
        plate: 0,
        transform: if index == 1 {
            Transform::from_translation(Vec3::new(1.0, 2.0, 3.0))
                .turned(core_geometry::Quat::from_rotation_x(0.6))
                .stretched(Vec3::new(1.0, 1.0, 1.5))
        } else {
            Transform::from_translation(Vec3::new(1.0, 2.0, 3.0))
        },
        visible: true,
        summary: Summary {
            vertices_merged: 7,
            faces_removed: 1,
            orientation: Orientation {
                flipped_faces: 2,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: MeshDiagnostics {
                vertices: 3,
                faces: 1,
                degenerate_faces: 0,
                duplicate_faces: 0,
                unreferenced_vertices: 0,
                boundary_edges: 3,
                holes: 1,
                non_manifold_edges: 0,
                unbalanced_edges: 3,
                shells: 1,
                euler_characteristic: 1,
            },
        },
        supports: ObjectSupportState {
            points: vec![SupportPoint::new(Vec3::Z).in_group(1)],
            painted: marked(&[0]),
            blocked: Region::default(),
            frozen: Vec::new(),
            grown: Vec::new(),
            raft_lift_mm: 0.5,
        },
        hollow: ObjectHollowState {
            blockers: vec![Blocker::ball(Vec3::ZERO, 1.5)],
            drains: Vec::new(),
            channels: vec![Channel {
                points: vec![Vec3::ZERO, Vec3::Z],
                diameter_mm: 3.0,
                ..Channel::default()
            }],
            built: Some(BuiltCavity {
                wall: Cavity {
                    thickness_mm: 1.5,
                    mode: HollowMode::External,
                    precision: 0.25,
                    infill: None,
                    open_toward: None,
                },
                cavity_faces: 0..1,
                cavity_mm3: 12.5,
                voxel_mm: 0.2,
                coarsened: false,
                scale: Vec3::ONE,
                stretched: (index == 1).then_some(Mat3::from_cols(
                    Vec3::new(2.0, 0.0, 0.0),
                    Vec3::new(0.5, 1.0, 0.0),
                    Vec3::new(0.0, 0.25, 1.5),
                )),
            }),
        },
        own_hollow: None,
        own_drain: (index == 1).then_some(DrainState::round(4.0, 3.0, 1.0)),
        own_supports: if index == 1 {
            vec![OwnSupportState {
                group: 1,
                profile: SupportProfile::shipped("heavy").expect("the heavy preset ships"),
            }]
        } else {
            Vec::new()
        },
        own_raft: None,
        plug_of: None,
        relief_painted: marked(&[0]),
    }
}

fn marked(faces: &[usize]) -> Region {
    let mut region = Region::default();
    for face in faces {
        region.set(*face, true);
    }
    region
}

fn round_trip(project: &Project) -> Project {
    let mut bytes = Cursor::new(Vec::new());
    write_to(&mut bytes, project).expect("a project in memory writes");
    bytes.set_position(0);
    read_from(bytes).expect("what was just written reads")
}

#[test]
fn a_plate_comes_back_as_it_was_saved() {
    let saved = project(2);
    let back = round_trip(&saved);

    assert_eq!(back.manifest.objects.len(), 2);
    assert_eq!(back.models[0].source, saved.models[0].source);
    let object = &back.manifest.objects[0];
    assert_eq!(object.transform, saved.manifest.objects[0].transform);
    assert_eq!(
        object.supports.points,
        saved.manifest.objects[0].supports.points
    );
    assert_eq!(
        object.supports.painted,
        saved.manifest.objects[0].supports.painted
    );
    assert_eq!(
        object.relief_painted,
        saved.manifest.objects[0].relief_painted
    );
    assert_eq!(
        object.hollow.channels,
        saved.manifest.objects[0].hollow.channels
    );
    assert_eq!(back.manifest.slicing.format, OutputFormat::Ctb5);
    assert_eq!(back.manifest.hollow.mode, HollowMode::External);
}

#[test]
fn a_painted_region_counts_its_own_bits_whatever_count_the_file_claims() {
    let mut region: Region = serde_json::from_str(r#"{ "words": [5], "marked": 0 }"#)
        .expect("a region with a wrong count still reads");
    assert_eq!(region.len(), 2, "bits 0 and 2 are set in the word 5");
    assert!(region.set(0, false));
    assert_eq!(region.len(), 1);
}

#[test]
fn a_hollowed_model_brings_its_shell_back_with_what_it_was_measured_at() {
    let saved = project(1);
    let back = round_trip(&saved);

    assert_eq!(back.models[0].shell, saved.models[0].shell);
    assert_eq!(
        back.manifest.objects[0].hollow.built,
        saved.manifest.objects[0].hollow.built
    );
}

#[test]
fn the_cut_joints_and_parts_come_back_and_a_file_without_them_reads_the_defaults() {
    let mut saved = project(1);
    saved.manifest.cut.joints.kind = Some(core_joint::JointKind::Magnets);
    saved.manifest.cut.joints.magnets.thickness_mm = 3.0;
    saved.manifest.cut.parts.smallest_mm3 = 5.0;
    let back = round_trip(&saved);
    assert_eq!(back.manifest.cut.joints, saved.manifest.cut.joints);
    assert_eq!(back.manifest.cut.parts, saved.manifest.cut.parts);

    let older: CutState =
        serde_json::from_str(r#"{ "axis": "z", "height_mm": 10.0, "keep": "both" }"#)
            .expect("a cut written before the joints reads");
    assert_eq!(older.joints.kind, None);
    assert_eq!(older.parts, PartSettings::default());
}

#[test]
fn a_support_tool_written_before_lines_reads_the_default_spacing() {
    let mut state =
        serde_json::to_value(&project(1).manifest.supports).expect("the support tool serialises");
    state
        .as_object_mut()
        .expect("the support tool is an object")
        .remove("inline_spacing_mm");
    let older: SupportState =
        serde_json::from_value(state).expect("a tool written before lines reads");
    assert!((older.inline_spacing_mm - core_supports::INLINE_SPACING_MM).abs() < f32::EPSILON);
}

#[test]
fn a_summary_written_before_holes_were_counted_reads_none() {
    let mut diagnostics = serde_json::to_value(&project(1).manifest.objects[0].summary.diagnostics)
        .expect("the diagnostics serialise");
    diagnostics
        .as_object_mut()
        .expect("the diagnostics are an object")
        .remove("holes");
    let older: MeshDiagnostics =
        serde_json::from_value(diagnostics).expect("a summary written before holes reads");
    assert_eq!(older.holes, 0);
}

#[test]
fn a_fill_written_before_offsets_reads_as_it_was_filled() {
    let older: ProjectSettings =
        serde_json::from_str(r#"{ "infill_spacing_mm": 3.0, "border_spacing_mm": 2.0 }"#)
            .expect("a fill written before offsets reads");
    assert_eq!(older.border_offset_mm, 0.0, "the rim stood on the edge");
    assert_eq!(older.infill_offset_mm, 0.0, "the grid ran up to the edge");
    assert_eq!(older.second_border_mm, None);
    assert_eq!(older.pattern, core_supports::InfillPattern::Grid);
}

#[test]
fn a_solid_model_carries_one_mesh() {
    let mut saved = project(1);
    saved.manifest.objects[0].hollow.built = None;
    saved.models[0].shell = None;

    let back = round_trip(&saved);
    assert!(back.models[0].shell.is_none());
    assert_eq!(back.models[0].printed(), &back.models[0].source);
}

#[test]
fn a_models_own_settings_come_back_and_a_file_without_them_reads() {
    let back = round_trip(&project(2));
    assert_eq!(back.manifest.objects[0].own_drain, None);
    assert_eq!(
        back.manifest.objects[1]
            .own_drain
            .as_ref()
            .map(|own| own.diameter_mm),
        Some(4.0)
    );
    let mut older = serde_json::to_value(&back.manifest.objects[1]).expect("serialises");
    let fields = older.as_object_mut().expect("an object");
    fields.remove("own_drain");
    let read: ObjectState = serde_json::from_value(older).expect("a file from before reads");
    assert_eq!(read.own_drain, None);
}

#[test]
fn a_model_is_built_to_its_own_support_profile_where_it_keeps_one() {
    let plate = [
        SupportProfile::shipped("light").expect("the light preset ships"),
        SupportProfile::shipped("medium").expect("the medium preset ships"),
    ];
    assert_eq!(object(0).support_table(&plate, None), plate.to_vec());
    let heavy = SupportProfile {
        raft: plate[0].raft,
        ..SupportProfile::shipped("heavy").expect("the heavy preset ships")
    };
    assert_eq!(
        object(1).support_table(&plate, None),
        vec![
            SupportProfile::shipped("light").expect("the light preset ships"),
            heavy
        ],
        "the second model keeps its own profile of group 1, on the first group's raft"
    );
}

#[test]
fn a_model_stands_on_its_own_raft_then_the_plates_then_the_first_groups() {
    let plate = [
        SupportProfile::shipped("light").expect("the light preset ships"),
        SupportProfile::shipped("medium").expect("the medium preset ships"),
    ];
    let thick = Raft {
        enabled: true,
        thickness_mm: 3.5,
        ..plate[0].raft
    };
    let thicker = Raft {
        thickness_mm: 4.5,
        ..thick
    };

    assert_eq!(object(0).support_table(&plate, None)[0].raft, plate[0].raft);
    assert_eq!(object(0).support_table(&plate, Some(thick))[0].raft, thick);
    let mut own = object(0);
    own.own_raft = Some(thicker);
    assert_eq!(own.support_table(&plate, Some(thick))[0].raft, thicker);
    assert_eq!(
        own.support_table(&plate, Some(thick))[1].raft,
        thicker,
        "every group stands on the one raft"
    );
    let mut second = plate.clone();
    second[1].raft = thick;
    assert_eq!(
        object(0).support_table(&second, None)[1].raft,
        plate[0].raft,
        "with none given, every group takes the first group's"
    );
}

#[test]
fn a_models_own_support_profile_comes_back_and_a_file_without_one_reads() {
    let back = round_trip(&project(2));
    assert!(back.manifest.objects[0].own_supports.is_empty());
    assert_eq!(
        back.manifest.objects[1].own_supports,
        vec![OwnSupportState {
            group: 1,
            profile: SupportProfile::shipped("heavy").expect("the heavy preset ships"),
        }]
    );
    let mut older = serde_json::to_value(&back.manifest.objects[1]).expect("serialises");
    let fields = older.as_object_mut().expect("an object");
    fields.remove("own_supports");
    let read: ObjectState = serde_json::from_value(older).expect("a file from before reads");
    assert!(read.own_supports.is_empty());
}

#[test]
fn a_stretched_model_comes_back_stretched_and_a_file_from_before_reads() {
    let saved = project(2);
    let back = round_trip(&saved);
    let (object, was) = (&back.manifest.objects[1], &saved.manifest.objects[1]);
    assert_eq!(object.transform, was.transform);
    assert_eq!(object.hollow.built, was.hollow.built);

    let mut older = serde_json::to_value(&back.manifest.objects[0]).expect("serialises");
    assert!(
        older["transform"].get("stretch").is_none(),
        "nothing stretched writes the transform as a file from before has it"
    );
    let built = older["hollow"]["built"].as_object_mut().expect("a shell");
    assert!(built.remove("stretched").is_none());
    built.insert("scale".to_owned(), serde_json::json!([-2.0, 1.0, 0.5]));
    let read: ObjectState = serde_json::from_value(older).expect("a file from before reads");
    assert_eq!(read.transform.stretch, Mat3::IDENTITY);
    assert_eq!(
        read.hollow.built.map(|built| built.shape()),
        Some(Mat3::from_diagonal(Vec3::new(2.0, 1.0, 0.5))),
        "an older shell was measured under the size of its scale"
    );
}

#[test]
fn hole_shapes_an_open_floor_and_plugs_come_back_and_a_file_from_before_reads() {
    let mut saved = project(2);
    saved.manifest.objects[1].plug_of = Some(0);
    saved.manifest.drain.shape = HoleShape::Polygon { sides: 6 };
    saved.manifest.drain.plug_gap_mm = 0.2;
    saved.manifest.hollow.open_floor = true;
    let back = round_trip(&saved);
    assert_eq!(back.manifest.drain, saved.manifest.drain);
    assert!(back.manifest.hollow.open_floor);
    assert_eq!(back.manifest.objects[1].plug_of, Some(0));

    let older: DrainState =
        serde_json::from_str(r#"{ "diameter_mm": 3.0, "depth_mm": 3.0, "taper": 1.0 }"#)
            .expect("a drain written before ADR 0313 reads");
    assert_eq!(older, DrainState::round(3.0, 3.0, 1.0));
    let hole: DrainHole = serde_json::from_value(serde_json::json!({
        "at": [0.0, 0.0, 0.0], "axis": [0.0, 0.0, -1.0], "diameter_mm": 3.0,
        "depth_mm": 3.0, "taper": 1.0, "lift_mm": 0.0
    }))
    .expect("a hole written before ADR 0313 reads");
    assert_eq!(hole.shape, HoleShape::Round);
    assert!(
        hole.kind == core_volume::HoleKind::Drain,
        "a hole written before ADR 0346 drains, as it was cut"
    );
}

#[test]
fn infill_saved_before_walls_had_their_own_size_reads() {
    let older = serde_json::json!({ "pattern": "grid", "size_mm": 5.0, "density": 0.15 });
    let read: InfillSettings = serde_json::from_value(older).expect("a file from before reads");
    assert_eq!(read.pattern, InfillPattern::Grid);
    assert_eq!(read.cell_mm, 5.0, "the old cell size carries over");
    assert_eq!(read.density, 0.15);
}

#[test]
fn infill_saved_with_a_wall_of_its_own_reads_as_the_density_it_made() {
    // 6 mm cells and 2 mm walls on an 8 mm pitch leave 1 - (6/8)² = 7/16 solid.
    let older = serde_json::json!({
        "pattern": "grid", "cell_mm": 6.0, "wall_mm": 2.0, "density": 0.3, "hole_mm": 2.5
    });
    let read: InfillSettings = serde_json::from_value(older).expect("a file from before reads");
    assert!(
        (read.density - 7.0 / 16.0).abs() < 1e-6,
        "got {}",
        read.density
    );
    assert!(
        (read.wall_mm() - 2.0).abs() < 1e-4,
        "and it gives the wall back"
    );
    let scaffold = serde_json::json!({ "pattern": "scaffold", "wall_mm": 2.0, "density": 0.3 });
    let read: InfillSettings = serde_json::from_value(scaffold).expect("a file from before reads");
    assert_eq!(read.density, 0.3, "a scaffold never had a wall of its own");
    let hive = serde_json::json!({ "pattern": "hive", "cell_mm": 6.0, "wall_mm": 2.0 });
    let read: InfillSettings = serde_json::from_value(hive).expect("a file from before reads");
    assert!(
        (read.wall_mm() - 2.0).abs() < 1e-4,
        "a hive gives its wall back too"
    );
    let empty = serde_json::json!({ "pattern": "grid", "wall_mm": 0.0, "density": 0.3 });
    let read: InfillSettings = serde_json::from_value(empty).expect("a file from before reads");
    assert_eq!(
        read.density, 0.3,
        "a wall of no thickness is no density to read"
    );
}

#[test]
fn an_empty_plate_round_trips() {
    let back = round_trip(&project(0));
    assert!(back.manifest.objects.is_empty());
    assert!(back.models.is_empty());
}

#[test]
fn a_version_5_file_reads_its_lattice_size_as_the_cell() {
    let mut saved = project(1);
    saved.manifest.version = ONE_LATTICE_SIZE;
    let mut bytes = Cursor::new(Vec::new());
    write_to(&mut bytes, &saved).expect("a project in memory writes");
    bytes.set_position(0);
    let back = read_from(bytes).expect("a version 5 file reads");
    assert_eq!(
        back.manifest.version, VERSION,
        "and is this build's once open"
    );
    assert_eq!(back.manifest.hollow, saved.manifest.hollow);
}

#[test]
fn a_project_of_another_format_version_is_refused() {
    for found in [WHOLE_PROFILES - 1, VERSION + 1] {
        let mut saved = project(1);
        saved.manifest.version = found;

        let mut bytes = Cursor::new(Vec::new());
        write_to(&mut bytes, &saved).expect("a project in memory writes");
        bytes.set_position(0);

        let error = read_from(bytes).expect_err("the format is not this one");
        assert!(matches!(error, ProjectError::WrongVersion { found: it } if it == found));
    }
}

#[test]
fn a_plate_brings_its_printer_by_id_and_its_picture_back() {
    let saved = project(1);
    let back = round_trip(&saved);
    assert_eq!(back.manifest.printer, saved.manifest.printer);
    assert_eq!(back.manifest.resin, None);
    assert_eq!(back.preview, saved.preview);

    let mut bare = project(1);
    bare.preview = None;
    assert_eq!(round_trip(&bare).preview, None, "a file without one reads");
}

#[test]
fn a_version_4_file_keeps_the_ids_of_the_profiles_it_carried() {
    let saved = project(1);
    let mut older = serde_json::to_value(&saved.manifest).expect("serialises");
    older["version"] = WHOLE_PROFILES.into();
    older["printer"] = serde_json::json!({ "id": "my-saturn", "profile": { "name": "Mine" } });
    older["resin"] = serde_json::json!({ "id": null, "profile": { "name": "From a file" } });
    let bytes = archive(&older, &saved);

    let back = read_from(bytes).expect("a version 4 file reads");
    assert_eq!(back.manifest.version, VERSION);
    assert_eq!(back.manifest.printer.as_deref(), Some("my-saturn"));
    assert_eq!(back.manifest.resin, None, "a resin with no id names none");
    assert_eq!(back.preview, None);
}

#[test]
fn a_project_written_to_disk_reads_back() {
    let path = std::env::temp_dir().join("encrust-project-round-trip.encrust");
    let saved = project(1);
    save(&path, &saved).expect("the temporary directory is writable");

    let back = load(&path).expect("what was just written reads");
    assert_eq!(back.manifest.objects.len(), 1);
    assert_eq!(back.models[0].source, saved.models[0].source);
    std::fs::remove_file(&path).expect("the file was just written");
}

#[test]
fn a_file_that_is_not_a_zip_is_refused() {
    let error = read_from(Cursor::new(b"# not a project".to_vec()))
        .expect_err("a text file is not a project");
    assert!(matches!(error, ProjectError::NotAProject(_)));
}

#[test]
fn a_zip_without_a_manifest_is_refused() {
    let mut bytes = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut bytes);
    zip.start_file("readme.txt", zip::write::SimpleFileOptions::default())
        .expect("a zip in memory takes an entry");
    zip.finish().expect("a zip in memory finishes");
    bytes.set_position(0);

    let error = read_from(bytes).expect_err("there is no manifest in it");
    assert!(matches!(error, ProjectError::NotAProject(_)));
}

#[test]
fn a_project_missing_a_mesh_is_refused() {
    let saved = project(2);
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut bytes);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file(MANIFEST, options)
            .expect("a zip in memory takes an entry");
        zip.write_all(&serde_json::to_vec(&saved.manifest).expect("the manifest serialises"))
            .expect("a zip in memory takes bytes");
        zip.start_file(source_name(0), options)
            .expect("a zip in memory takes an entry");
        zip.write_all(&blob::write(&saved.models[0].source))
            .expect("a zip in memory takes bytes");
        zip.finish().expect("a zip in memory finishes");
    }
    bytes.set_position(0);

    let error = read_from(bytes).expect_err("the first model has no shell in the file");
    assert!(matches!(error, ProjectError::BadMesh { .. }));
}

/// An archive of `manifest` as written by hand, holding `saved`'s first model as its source
/// and shell.
fn archive(manifest: &serde_json::Value, saved: &Project) -> Cursor<Vec<u8>> {
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut bytes);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file(MANIFEST, options)
            .expect("a zip in memory takes an entry");
        zip.write_all(&serde_json::to_vec(manifest).expect("the manifest serialises"))
            .expect("a zip in memory takes bytes");
        for (name, mesh) in [
            (source_name(0), &saved.models[0].source),
            (shell_name(0), &saved.models[0].source),
        ] {
            zip.start_file(name, options)
                .expect("a zip in memory takes an entry");
            zip.write_all(&blob::write(mesh))
                .expect("a zip in memory takes bytes");
        }
        zip.finish().expect("a zip in memory finishes");
    }
    bytes.set_position(0);
    bytes
}

#[test]
fn a_version_6_file_reads_and_leaves_its_print_settings_to_the_profiles() {
    let saved = project(1);
    let mut older = serde_json::to_value(&saved.manifest).expect("serialises");
    older["version"] = PRINT_SETTINGS_KEPT.into();
    older["slicing"]["layer_height_mm"] = 0.03.into();
    older["slicing"]["anti_alias"] = false.into();
    older["slicing"]["exposure"] = serde_json::json!([]);

    let back = read_from(archive(&older, &saved)).expect("a version 6 file reads");
    assert_eq!(back.manifest.version, VERSION);
    assert_eq!(back.manifest.slicing, saved.manifest.slicing);
}

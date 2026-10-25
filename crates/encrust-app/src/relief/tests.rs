use super::*;

fn scene_with_a_tetrahedron() -> Scene {
    use core_geometry::{Mesh, Orientation, Transform, diagnose};
    let mesh = Mesh::new(
        vec![
            Vec3::ZERO,
            Vec3::new(20.0, 0.0, 0.0),
            Vec3::new(0.0, 20.0, 0.0),
            Vec3::new(0.0, 0.0, 20.0),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
    );
    let mut scene = Scene::default();
    scene.insert(crate::scene::Imported::new(
        "tetrahedron".to_owned(),
        Arc::new(mesh.clone()),
        Transform::default(),
        crate::scene::ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&mesh),
        },
    ));
    scene
}

#[test]
fn a_picture_is_read_as_its_grey_and_kept_by_name() {
    let mut png = Vec::new();
    let white = image::GrayImage::from_pixel(2, 3, image::Luma([255]));
    white
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("a png writes into memory");
    let file = Handed::bytes("stone.png", Arc::from(png));

    let mut tool = ReliefTool::default();
    load_picture(&mut tool, &mut Status::default(), &file);
    let picture = tool.picture.expect("a png reads");
    assert_eq!(picture.name, "stone.png");
    assert_eq!(picture.size_px, [2, 3]);
    assert!((picture.heights.sample(core_geometry::Vec2::splat(0.5)) - 1.0).abs() < 1e-6);
}

#[test]
fn a_builtin_texture_is_chosen_by_its_place_in_the_list() {
    let mut tool = ReliefTool::default();
    choose_builtin(&mut tool, &mut Status::default(), 2);
    let picture = tool.picture.expect("a shipped texture reads");
    assert_eq!(picture.builtin, Some(2));
    assert_eq!(picture.name, (BUILTINS[2].name)());
}

#[test]
fn a_file_that_is_no_picture_is_said_so() {
    let mut tool = ReliefTool::default();
    let mut status = Status::default();
    let file = Handed::bytes("notes.png", Arc::from(&b"not a picture"[..]));
    load_picture(&mut tool, &mut status, &file);
    assert!(tool.picture.is_none());
    assert!(status.is_error());
}

#[test]
fn a_picture_presses_into_a_model_with_no_texture() {
    let mut tool = ReliefTool::default();
    let scene = scene_with_a_tetrahedron();
    assert!(
        tool.blocker(&scene).is_some(),
        "an untextured cube has nothing to press"
    );
    tool.picture = Some(Picture {
        name: "flat.png".to_owned(),
        heights: Arc::new(Heightmap::new(1, 1, vec![1.0]).expect("one pixel")),
        size_px: [1, 1],
        builtin: None,
    });
    assert_eq!(tool.blocker(&scene), None);
}

#[test]
fn an_empty_plate_says_there_is_nothing_to_press_into() {
    let mut tool = ReliefTool::default();
    let scene = Scene::default();
    assert_eq!(
        tool.blocker(&scene).as_deref(),
        Some("Nothing visible on the plate to press a texture into.")
    );
    assert!(tool.start(&scene).is_err());
}

#[test]
fn a_chosen_picture_is_laid_over_the_models_it_would_press_and_kept_while_nothing_changes() {
    let scene = scene_with_a_tetrahedron();
    let mut tool = ReliefTool::default();
    assert!(
        tool.textures(&scene).is_empty(),
        "the tetrahedron carries no texture of its own"
    );

    tool.picture = Some(Picture {
        name: "stone.png".to_owned(),
        heights: Arc::new(Heightmap::new(1, 1, vec![1.0]).expect("one pixel")),
        size_px: [1, 1],
        builtin: None,
    });
    let first = tool.textures(&scene);
    let laid = first
        .values()
        .next()
        .expect("the picture is laid over the model");
    assert_eq!(
        laid.uvs.mapped(),
        4,
        "every face of the tetrahedron is mapped"
    );
    let again = tool.textures(&scene);
    assert!(
        Arc::ptr_eq(laid, again.values().next().expect("still laid")),
        "nothing changed, so the view keeps what it uploaded"
    );

    tool.tiling.tile_mm *= 2.0;
    let retiled = tool.textures(&scene);
    assert!(!Arc::ptr_eq(
        laid,
        retiled.values().next().expect("still laid")
    ));
}

#[test]
fn a_painted_model_is_drawn_and_pressed_only_where_it_is_painted() {
    let mut scene = scene_with_a_tetrahedron();
    let mut tool = ReliefTool {
        picture: Some(Picture {
            name: "stone.png".to_owned(),
            heights: Arc::new(Heightmap::new(1, 1, vec![1.0]).expect("one pixel")),
            size_px: [1, 1],
            builtin: None,
        }),
        ..ReliefTool::default()
    };
    let whole = tool.textures(&scene);
    let whole = whole.values().next().expect("the picture is laid");
    assert_eq!(whole.uvs.mapped(), 4, "nothing painted lays it everywhere");

    tool.brushing = true;
    let none = tool.textures(&scene);
    let none = none.values().next().expect("still laid");
    assert_eq!(
        none.uvs.mapped(),
        0,
        "with the brush in hand, nothing painted is nowhere"
    );
    assert_eq!(tool.blocker(&scene), Some(tr!("relief-nothing-painted")));

    scene.objects_mut()[0].relief_painted.set(2, true);
    let painted = tool.textures(&scene);
    let painted = painted.values().next().expect("still laid");
    assert_eq!(
        painted.uvs.mapped(),
        1,
        "only the painted face is drawn textured"
    );
    assert!(painted.uvs.of_face(2).is_some());
    let tasks = relief_tasks(&scene, true, false);
    assert_eq!(
        tasks[0].painted.as_ref().map(Region::len),
        Some(1),
        "the press is handed the painted face"
    );
}

#[test]
fn a_face_is_laid_from_the_plane_it_faces_at_the_tile_size() {
    let picture = Picture {
        name: "stone.png".to_owned(),
        heights: Arc::new(Heightmap::new(1, 1, vec![1.0]).expect("one pixel")),
        size_px: [1, 1],
        builtin: None,
    };
    // A face in the plane z = 0, so it is read from above, 10 mm on a side.
    let mesh = Mesh::new(
        vec![
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(0.0, 10.0, 0.0),
        ],
        vec![[0, 1, 2]],
    );
    let tiling = Tiling {
        tile_mm: 5.0,
        angle_deg: 0.0,
    };
    let laid = lay(&picture, &mesh, Mat3::IDENTITY, tiling);
    let corners = laid.uvs.of_face(0).expect("the face is mapped");
    assert_eq!(
        corners.corners,
        [Vec2::ZERO, Vec2::new(2.0, 0.0), Vec2::new(0.0, 2.0)],
        "10 mm over 5 mm tiles is two tiles"
    );
}

#[test]
fn the_settings_are_what_the_panel_is_set_to() {
    let tool = ReliefTool {
        state: ReliefState {
            amplitude_mm: -0.3,
            precision: 0.8,
            ..ReliefState::default()
        },
        ..ReliefTool::default()
    };
    let settings = tool.settings();
    assert!((settings.amplitude_mm + 0.3).abs() < 1e-6);
    assert!((settings.precision - 0.8).abs() < 1e-6);
}

#[test]
fn an_inverted_picture_is_drawn_the_way_it_is_pressed() {
    let scene = scene_with_a_tetrahedron();
    let mut tool = ReliefTool {
        picture: Some(Picture {
            name: "white.png".to_owned(),
            heights: Arc::new(Heightmap::new(1, 1, vec![1.0]).expect("one pixel")),
            size_px: [1, 1],
            builtin: None,
        }),
        state: ReliefState {
            invert: true,
            ..ReliefState::default()
        },
        ..ReliefTool::default()
    };
    let drawn = tool.textures(&scene);
    let laid = drawn
        .values()
        .next()
        .expect("the picture is laid over the model");
    assert!(
        laid.heights[0].sample(Vec2::splat(0.5)).abs() < 1e-6,
        "white turned over is drawn black, as it would press"
    );
}

#[test]
fn a_turned_model_holds_the_faces_that_face_the_plate_once_turned() {
    let mut scene = scene_with_a_tetrahedron();
    let id = scene.objects()[0].id;
    // A quarter turn about X stands the model's +Y up on the plate.
    scene
        .get_mut(id)
        .expect("the tetrahedron is in the scene")
        .transform
        .rotation = core_geometry::Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let tasks = relief_tasks(&scene, true, false);
    let up = tasks[0].up;
    assert!(
        (up - Vec3::Y).length() < 1e-5,
        "got {up}, expected the model's +Y"
    );
}

#[test]
fn a_hollow_model_keeps_its_blockers_through_a_press_and_is_hollowed_again() {
    let mut scene = scene_with_a_tetrahedron();
    let mut status = Status::default();
    let mut hollow = crate::hollow::HollowTool::default();
    hollow.start(&scene).expect("a model is on the plate");
    while hollow.poll(&mut scene, &mut status) {
        std::thread::yield_now();
    }
    let object = &mut scene.objects_mut()[0];
    assert!(object.hollow.is_hollow(), "{status:?}");
    object
        .hollow
        .add_blocker(Vec3::new(5.0, 5.0, 5.0), 2.0, object.transform);

    let mut tool = ReliefTool {
        picture: Some(Picture {
            name: "flat.png".to_owned(),
            heights: Arc::new(Heightmap::new(1, 1, vec![1.0]).expect("one pixel")),
            size_px: [1, 1],
            builtin: None,
        }),
        ..ReliefTool::default()
    };
    tool.start(&scene)
        .expect("the picture presses into the tetrahedron");
    while tool.poll(&mut scene, &mut status) {
        std::thread::yield_now();
    }
    let object = &scene.objects()[0];
    assert!(
        !object.hollow.is_hollow(),
        "the old cavity was cut into the old surface"
    );
    assert_eq!(object.hollow.blockers().len(), 1, "the blocker stays");
    assert_eq!(tool.unhollowed.len(), 1);

    assert!(hollow.again(&scene, &tool.unhollowed));
    while hollow.poll(&mut scene, &mut status) {
        std::thread::yield_now();
    }
    assert!(scene.objects()[0].hollow.is_hollow(), "{status:?}");
}

#[test]
fn a_relief_pressed_into_a_model_resized_meanwhile_is_dropped() {
    let mut scene = scene_with_a_tetrahedron();
    let mut tool = ReliefTool {
        picture: Some(Picture {
            name: "flat.png".to_owned(),
            heights: Arc::new(Heightmap::new(1, 1, vec![1.0]).expect("one pixel")),
            size_px: [1, 1],
            builtin: None,
        }),
        ..ReliefTool::default()
    };
    tool.start(&scene)
        .expect("the picture presses into the tetrahedron");
    let object = &mut scene.objects_mut()[0];
    let before = Arc::clone(&object.mesh);
    object.transform.scale = Vec3::splat(2.0);
    let mut status = Status::default();
    while tool.poll(&mut scene, &mut status) {
        std::thread::yield_now();
    }
    assert_eq!(
        status,
        Status::Error(tr!("job-model-changed", name = "tetrahedron"))
    );
    let object = &scene.objects()[0];
    assert!(Arc::ptr_eq(&object.mesh, &before), "the mesh is left alone");
    assert_eq!(
        object.transform.scale,
        Vec3::splat(2.0),
        "and so is its size"
    );
}

#[test]
fn the_mode_and_texture_chosen_come_back_in_the_next_run() {
    let mut tool = ReliefTool {
        mode: ReliefMode::Combine,
        ..ReliefTool::default()
    };
    choose_builtin(&mut tool, &mut Status::default(), 2);
    let text = serde_json::to_string(&tool.choice()).expect("the choice serialises");
    let read: ReliefChoice = serde_json::from_str(&text).expect("and reads back");
    let mut next = ReliefTool::default();
    next.restore(&read).expect("a built-in texture reads");
    assert_eq!(next.mode, ReliefMode::Combine);
    assert_eq!(next.picture.and_then(|picture| picture.builtin), Some(2));
}

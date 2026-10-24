use super::*;
use core_geometry::{Aabb, Mapping, Vec2, Vec3};

const NEVER: Cancel<'static> = Cancel::never();

/// A closed cube of `side` millimetres standing on the origin, with every face's UVs
/// taken from where its corners stand in X and Y.
fn cube(side: Scalar) -> (Mesh, UvMap) {
    let corners = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(side, 0.0, 0.0),
        Vec3::new(side, side, 0.0),
        Vec3::new(0.0, side, 0.0),
        Vec3::new(0.0, 0.0, side),
        Vec3::new(side, 0.0, side),
        Vec3::new(side, side, side),
        Vec3::new(0.0, side, side),
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

    let mesh = Mesh::new(corners.to_vec(), faces);
    let planar = |vertex: Vec3| Vec2::new(vertex.x / side, vertex.y / side);
    let uvs = UvMap::whole(
        mesh.faces
            .iter()
            .map(|face| face.map(|index| planar(corners[index as usize])))
            .collect(),
    );
    (mesh, uvs)
}

fn flat(height: Scalar) -> Heightmap {
    Heightmap::new(1, 1, vec![height]).expect("one sample over one pixel")
}

/// Black over the left half of the unit square, white over the right, with a pixel
/// either side of each boundary so that both halves have a plateau of their own.
fn half() -> Heightmap {
    Heightmap::new(4, 1, vec![0.0, 0.0, 1.0, 1.0]).expect("four samples over four pixels")
}

fn pressed_cube(heights: &Heightmap, amplitude_mm: Scalar) -> Mesh {
    let (mesh, uvs) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    press(
        &mesh,
        &bvh,
        &uvs,
        std::slice::from_ref(heights),
        &ReliefSettings {
            amplitude_mm,
            precision: 0.5,
            ..ReliefSettings::default()
        },
    )
    .expect("a mapped cube can be pressed")
    .mesh
}

fn bounds(mesh: &Mesh) -> Aabb {
    mesh.aabb().expect("the pressed mesh has faces")
}

/// What the lattice itself can promise: marching the cells places the surface within
/// half a voxel, and `ReliefSettings` cuts a 0.4 mm amplitude at 0.13 mm.
const TOLERANCE_MM: Scalar = 0.1;

#[test]
fn a_white_picture_tiled_over_a_cube_moves_every_side_out() {
    let (mesh, _) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let settings = ReliefSettings {
        amplitude_mm: 0.4,
        ..ReliefSettings::default()
    };
    let pressed = press_tiled(&mesh, &bvh, &flat(1.0), Tiling::default(), &settings)
        .expect("any closed mesh can be tiled");
    let grown = bounds(&pressed.mesh);
    // Every side reads white from the plane it faces, so all six move out 0.4 mm.
    assert!(
        (grown.maxs.x - 10.4).abs() < TOLERANCE_MM && (grown.mins.y + 0.4).abs() < TOLERANCE_MM,
        "got {grown:?}"
    );
}

#[test]
fn a_tile_of_no_size_is_refused() {
    let (mesh, _) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let tiling = Tiling {
        tile_mm: 0.0,
        angle_deg: 0.0,
    };
    let refused = press_tiled(&mesh, &bvh, &flat(1.0), tiling, &ReliefSettings::default());
    assert!(matches!(refused, Err(VolumeError::BadTile(_))));
}

#[test]
fn a_white_texture_moves_the_whole_surface_out() {
    let grown = bounds(&pressed_cube(&flat(1.0), 0.4));
    assert!(
        (grown.maxs.z - 10.4).abs() < TOLERANCE_MM && (grown.mins.z + 0.4).abs() < TOLERANCE_MM,
        "got {grown:?}, expected a 10 mm cube grown 0.4 mm on every side"
    );
}

#[test]
fn a_negative_amplitude_sinks_the_surface_in() {
    let shrunk = bounds(&pressed_cube(&flat(1.0), -0.4));
    assert!(
        (shrunk.maxs.z - 9.6).abs() < TOLERANCE_MM,
        "got {shrunk:?}, expected a 10 mm cube taken 0.4 mm in on every side"
    );
}

#[test]
fn a_black_texture_leaves_the_surface_where_it_was() {
    let same = bounds(&pressed_cube(&flat(0.0), 0.4));
    assert!(
        (same.maxs.z - 10.0).abs() < TOLERANCE_MM && same.mins.z.abs() < TOLERANCE_MM,
        "got {same:?}, expected the 10 mm cube unmoved"
    );
}

#[test]
fn half_a_texture_moves_and_half_stays() {
    let mesh = pressed_cube(&half(), 0.4);
    // Inside a plateau of the texture, away from both the boundary between the halves
    // and the wrap at the edges of the square.
    let top = |from: Scalar, to: Scalar| {
        mesh.vertices
            .iter()
            .filter(|vertex| (from..to).contains(&vertex.x))
            .map(|vertex| vertex.z)
            .fold(Scalar::MIN, Scalar::max)
    };
    let white = top(6.5, 8.5);
    let black = top(1.5, 3.5);
    assert!(
        (white - 10.4).abs() < TOLERANCE_MM && (black - 10.0).abs() < TOLERANCE_MM,
        "white half topped out at {white} and black at {black}, expected 10.4 and 10.0"
    );
}

#[test]
fn a_pressed_solid_is_still_a_solid() {
    let pressed = pressed_cube(&flat(1.0), 0.4);
    assert!(
        core_geometry::diagnose(&pressed).is_closed(),
        "a relief comes out of the field as one closed surface"
    );
}

#[test]
fn a_face_the_map_does_not_cover_stays_where_it_was() {
    let (mesh, _) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let holed = UvMap::new(vec![None; mesh.faces.len()])
        .without(&[])
        .flipping(&[]);
    let Err(error) = press(
        &mesh,
        &bvh,
        &holed,
        &[flat(1.0)],
        &ReliefSettings::default(),
    ) else {
        panic!("a map that covers no face has nothing to press");
    };
    assert!(matches!(error, VolumeError::UnmappedMesh { mapped: 0, .. }));

    // One face mapped is enough to run, and the rest of the cube keeps its size.
    let mut corners = vec![None; mesh.faces.len()];
    corners[2] = Some(Mapping::sole([Vec2::ZERO; 3]));
    let pressed = press(
        &mesh,
        &bvh,
        &UvMap::new(corners),
        &[flat(1.0)],
        &ReliefSettings {
            amplitude_mm: 0.4,
            ..ReliefSettings::default()
        },
    )
    .expect("one mapped face is a map")
    .mesh;
    let bounds = bounds(&pressed);
    assert!(
        (bounds.mins.z).abs() < TOLERANCE_MM,
        "the unmapped underside stayed on the plate, got {}",
        bounds.mins.z
    );
}

#[test]
fn each_face_is_pressed_from_its_own_image() {
    let (mesh, _) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    // The lid off the first image, which is white; everything else off the second,
    // which is black.
    let faces = (0..mesh.faces.len())
        .map(|face| {
            Some(Mapping {
                texture: usize::from(!(2..4).contains(&face)),
                corners: [Vec2::ZERO; 3],
            })
        })
        .collect();

    let pressed = press(
        &mesh,
        &bvh,
        &UvMap::new(faces),
        &[flat(1.0), flat(0.0)],
        &ReliefSettings {
            amplitude_mm: 0.4,
            ..ReliefSettings::default()
        },
    )
    .expect("two images are two images")
    .mesh;

    let grown = bounds(&pressed);
    assert!(
        (grown.maxs.z - 10.4).abs() < TOLERANCE_MM && grown.mins.z.abs() < TOLERANCE_MM,
        "got {grown:?}, expected only the lid to have moved"
    );
}

#[test]
fn a_map_naming_an_image_that_was_not_given_is_refused() {
    let (mesh, uvs) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let second = UvMap::new(
        (0..mesh.faces.len())
            .map(|face| {
                uvs.of_face(face).map(|mapping| Mapping {
                    texture: 1,
                    ..mapping
                })
            })
            .collect(),
    );
    assert!(matches!(
        press(
            &mesh,
            &bvh,
            &second,
            &[flat(1.0)],
            &ReliefSettings::default()
        ),
        Err(VolumeError::MissingTexture {
            wanted: 2,
            given: 1
        })
    ));
}

#[test]
fn an_amplitude_of_nothing_is_refused() {
    let (mesh, uvs) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let settings = ReliefSettings {
        amplitude_mm: 0.0,
        ..ReliefSettings::default()
    };
    assert!(matches!(
        press(&mesh, &bvh, &uvs, &[flat(1.0)], &settings),
        Err(VolumeError::BadAmplitude(0.0))
    ));
}

#[test]
fn a_map_that_does_not_cover_the_mesh_is_refused() {
    let (mesh, _) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let short = UvMap::whole(vec![[Vec2::ZERO; 3]]);
    assert!(matches!(
        press(
            &mesh,
            &bvh,
            &short,
            &[flat(1.0)],
            &ReliefSettings::default()
        ),
        Err(VolumeError::UnmappedMesh {
            faces: 12,
            mapped: 1
        })
    ));
}

#[test]
fn an_empty_mesh_has_nothing_to_press_into() {
    let mesh = Mesh::default();
    let bvh = Bvh::build(&mesh);
    assert!(matches!(
        press(
            &mesh,
            &bvh,
            &UvMap::default(),
            &[flat(1.0)],
            &ReliefSettings::default()
        ),
        Err(VolumeError::EmptyMesh)
    ));
}

fn tiled_cube(heights: &Heightmap, settings: ReliefSettings) -> Aabb {
    let (mesh, _) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let pressed = press_tiled(&mesh, &bvh, heights, Tiling::default(), &settings)
        .expect("any closed mesh can be tiled");
    bounds(&pressed.mesh)
}

fn raised(settings: ReliefSettings) -> ReliefSettings {
    ReliefSettings {
        amplitude_mm: 0.4,
        ..settings
    }
}

#[test]
fn an_inverted_texture_raises_what_was_black() {
    let grown = tiled_cube(
        &flat(0.0),
        raised(ReliefSettings {
            invert: true,
            ..ReliefSettings::default()
        }),
    );
    assert!(
        (grown.maxs.z - 10.4).abs() < TOLERANCE_MM,
        "got {grown:?}, expected black read as white to grow the cube 0.4 mm"
    );
}

#[test]
fn a_symmetric_relief_leaves_mid_grey_where_it_was_and_moves_black_in() {
    let symmetric = raised(ReliefSettings {
        symmetric: true,
        ..ReliefSettings::default()
    });
    let grey = tiled_cube(&flat(0.5), symmetric);
    let black = tiled_cube(&flat(0.0), symmetric);
    assert!(
        (grey.maxs.z - 10.0).abs() < TOLERANCE_MM && (black.maxs.z - 9.8).abs() < TOLERANCE_MM,
        "grey topped out at {} and black at {}, expected 10.0 and half of 0.4 mm in",
        grey.maxs.z,
        black.maxs.z
    );
}

/// The cube with its lid widened to 14 mm, so that every side leans out over the
/// 10 mm bottom by about 11 degrees.
fn leaning_out() -> Mesh {
    let (cube, _) = cube(10.0);
    let vertices = cube
        .vertices
        .iter()
        .map(|corner| {
            if corner.z > 0.0 {
                Vec3::new(corner.x * 1.4 - 2.0, corner.y * 1.4 - 2.0, corner.z)
            } else {
                *corner
            }
        })
        .collect();
    Mesh::new(vertices, cube.faces)
}

#[test]
fn a_held_bottom_stays_flat_where_leaning_sides_meet_it() {
    let mesh = leaning_out();
    let bvh = Bvh::build(&mesh);
    let settings = raised(ReliefSettings {
        mask: AngleMask {
            bottom_deg: 5.0,
            ..AngleMask::default()
        },
        ..ReliefSettings::default()
    });
    let pressed = press_tiled(&mesh, &bvh, &flat(1.0), Tiling::default(), &settings)
        .expect("any closed mesh can be tiled");
    let grown = bounds(&pressed.mesh);
    // A side leaning out faces partly down, so without the clip at the held face's
    // plane its relief hangs up to 0.4 × sin 11° ≈ 0.08 mm below the bottom edge.
    assert!(
        grown.mins.z.abs() < TOLERANCE_MM / 2.0 && grown.maxs.x > 12.2,
        "got {grown:?}, expected the bottom at 0 and the sides moved out"
    );
}

#[test]
fn a_held_top_is_measured_from_the_up_it_is_given() {
    let grown = tiled_cube(
        &flat(1.0),
        raised(ReliefSettings {
            mask: AngleMask {
                up: Vec3::X,
                top_deg: 5.0,
                ..AngleMask::default()
            },
            ..ReliefSettings::default()
        }),
    );
    assert!(
        (grown.maxs.x - 10.0).abs() < TOLERANCE_MM && (grown.maxs.z - 10.4).abs() < TOLERANCE_MM,
        "got {grown:?}, expected the +X face held and the lid moved"
    );
}

#[test]
fn a_mask_past_level_or_with_no_up_is_refused() {
    let (mesh, _) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let with = |mask: AngleMask| {
        let settings = ReliefSettings {
            mask,
            ..ReliefSettings::default()
        };
        press_tiled(&mesh, &bvh, &flat(1.0), Tiling::default(), &settings)
    };
    assert!(matches!(
        with(AngleMask {
            bottom_deg: 95.0,
            ..AngleMask::default()
        }),
        Err(VolumeError::BadMaskAngle(_))
    ));
    assert!(matches!(
        with(AngleMask {
            top_deg: Scalar::NAN,
            ..AngleMask::default()
        }),
        Err(VolumeError::BadMaskAngle(_))
    ));
    assert!(matches!(
        with(AngleMask {
            up: Vec3::ZERO,
            ..AngleMask::default()
        }),
        Err(VolumeError::BadMaskUp(_))
    ));
}

#[test]
fn a_negative_smoothing_is_refused() {
    let (mesh, uvs) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let settings = ReliefSettings {
        smoothing_px: -1.0,
        ..ReliefSettings::default()
    };
    assert!(matches!(
        press(&mesh, &bvh, &uvs, &[flat(1.0)], &settings),
        Err(VolumeError::BadSmoothing(_))
    ));
}

#[test]
fn a_prepared_texture_is_smoothed_then_turned_over() {
    let settings = ReliefSettings {
        invert: true,
        smoothing_px: 4.0,
        ..ReliefSettings::default()
    };
    // Smoothed far past its four pixels, the half-black half-white strip is all
    // mid grey, which turned over is mid grey again.
    let strip = half();
    let read = prepared(&strip, &settings);
    assert!(
        read.samples()
            .iter()
            .all(|height| (height - 0.5).abs() < 0.05),
        "got {:?}",
        read.samples()
    );
    assert!(matches!(
        prepared(&strip, &ReliefSettings::default()),
        Cow::Borrowed(_)
    ));
}

#[test]
fn a_face_buried_in_another_shell_takes_no_relief() {
    // Two 10 mm cubes pushed half into each other, so that each has a side inside
    // the other: the solid is the 15 × 10 × 10 box they span.
    let mut mesh = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(10.0));
    mesh.append(&core_geometry::cuboid(
        Vec3::new(5.0, 0.0, 0.0),
        Vec3::new(15.0, 10.0, 10.0),
    ));
    let bvh = Bvh::build(&mesh);
    let settings = raised(ReliefSettings {
        symmetric: true,
        ..ReliefSettings::default()
    });
    let pressed = press_tiled(&mesh, &bvh, &flat(0.0), Tiling::default(), &settings)
        .expect("two closed cubes can be tiled");
    let depth = |p: Vec3| {
        (p - Vec3::ZERO)
            .min(Vec3::new(15.0, 10.0, 10.0) - p)
            .min_element()
    };
    let deepest = pressed
        .mesh
        .vertices
        .iter()
        .copied()
        .map(depth)
        .fold(Scalar::MIN, Scalar::max);
    // Black read symmetrically sinks the box's own sides by half the 0.4 mm amplitude.
    assert!(
        (deepest - 0.2).abs() < TOLERANCE_MM,
        "the deepest vertex stands {deepest} mm inside, expected only the sides sunk 0.2 mm"
    );
}

#[test]
fn a_kept_basis_presses_another_picture_into_the_same_model_as_a_fresh_one_would() {
    let (mesh, _) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let settings = raised(ReliefSettings::default());
    let tiled = |image: &Heightmap, basis| {
        press_tiled_reusing(
            &mesh,
            &bvh,
            image,
            Tiling::default(),
            &settings,
            basis,
            NEVER,
        )
        .expect("any closed mesh can be tiled")
    };
    let (_, basis) = tiled(&flat(1.0), None);
    let (reused, again) = tiled(&half(), Some(Arc::clone(&basis)));
    assert!(
        Arc::ptr_eq(&basis, &again),
        "the same model and lattice keep the basis"
    );
    let (fresh, _) = tiled(&half(), None);
    assert_eq!(reused.mesh, fresh.mesh);
}

#[test]
fn a_basis_kept_for_another_depth_or_model_is_built_again() {
    let (mesh, _) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let (_, basis) = press_tiled_reusing(
        &mesh,
        &bvh,
        &flat(1.0),
        Tiling::default(),
        &raised(ReliefSettings::default()),
        None,
        NEVER,
    )
    .expect("any closed mesh can be tiled");

    let deeper = ReliefSettings {
        amplitude_mm: 0.6,
        ..ReliefSettings::default()
    };
    let (_, rebuilt) = press_tiled_reusing(
        &mesh,
        &bvh,
        &flat(1.0),
        Tiling::default(),
        &deeper,
        Some(Arc::clone(&basis)),
        NEVER,
    )
    .expect("any closed mesh can be tiled");
    assert!(!Arc::ptr_eq(&basis, &rebuilt));

    let (larger, _) = cube(12.0);
    let (_, other) = press_tiled_reusing(
        &larger,
        &Bvh::build(&larger),
        &flat(1.0),
        Tiling::default(),
        &raised(ReliefSettings::default()),
        Some(Arc::clone(&basis)),
        NEVER,
    )
    .expect("any closed mesh can be tiled");
    assert!(!Arc::ptr_eq(&basis, &other));
}

#[test]
fn a_press_given_up_on_is_cancelled_and_keeps_no_basis() {
    let (mesh, uvs) = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let settings = raised(ReliefSettings::default());
    let stopped = || true;
    let pressed = press_reusing(
        &mesh,
        &bvh,
        &uvs,
        &[flat(1.0)],
        &settings,
        None,
        Cancel::when(&stopped),
    );
    assert!(matches!(pressed, Err(VolumeError::Cancelled)));

    let tiled = press_tiled_reusing(
        &mesh,
        &bvh,
        &flat(1.0),
        Tiling::default(),
        &settings,
        None,
        Cancel::when(&stopped),
    );
    assert!(matches!(tiled, Err(VolumeError::Cancelled)));
}

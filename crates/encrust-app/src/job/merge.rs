use std::sync::Arc;

use core_geometry::{Mesh, Transform, Vec3, transform_mesh};
use printer_profiles::Compensation;

use crate::scene::Scene;

/// Bakes every visible object of `plate`, and the supports under it, into one mesh in
/// plate coordinates, ready to slice.
///
/// The slicer takes a single mesh, and two objects that overlap are one solid on the
/// plate, so placement is applied here rather than layer by layer. A hollowed model goes
/// in as its shell — the outer surface with the cavity wound the other way — because the
/// rasteriser's positive winding rule is what takes the cavity out; see
/// `docs/decisions/0059`. Supports are already in plate coordinates and go in as they
/// are. Returns `None` when nothing visible has any geometry.
/// The same, with the resin's shrinkage compensation applied to each object as it goes in.
///
/// Each object is scaled about its own footprint and about the plate, not about the scene:
/// a part shrinks towards itself and is held at the plate while it prints, so a
/// correction must not move its neighbours; see `docs/design/compensation.md`.
pub fn merge_plate_compensated(
    scene: &Scene,
    plate: u32,
    compensation: &Compensation,
) -> Option<Mesh> {
    let mut merged = Mesh::default();

    for object in scene.printable(plate) {
        let mut part = Mesh::default();
        let mesh = object.hollow.shell().unwrap_or(&object.mesh);
        let placed = if object.transform == Transform::default() {
            mesh.as_ref().clone()
        } else {
            transform_mesh(mesh, object.transform)
        };
        append(&mut part, &placed);

        // The holes and channels, wound inward, which is what takes them out of the model
        // they were placed on, cavity or no cavity; see ADR 0075.
        if let Some(cuts) = object.hollow.cut_bodies() {
            append(&mut part, &transform_mesh(cuts, object.transform));
        }

        for supports in object.supports.meshes().unwrap_or_default() {
            append(&mut part, supports);
        }

        append(&mut merged, &compensated(part, compensation));
    }

    (!merged.is_empty()).then_some(merged)
}

/// `part` grown or shrunk to come out at the size it was modelled at, held where it
/// stands: the scale is taken about the middle of its footprint and about the plate.
fn compensated(part: Mesh, compensation: &Compensation) -> Mesh {
    let Some(bounds) = part.aabb().filter(|_| !compensation.scales_nothing()) else {
        return part;
    };
    let (scale, translation) = compensation.placement(
        (bounds.mins.x + bounds.maxs.x) / 2.0,
        (bounds.mins.y + bounds.maxs.y) / 2.0,
    );
    transform_mesh(
        &part,
        Transform {
            translation: Vec3::from_array(translation),
            scale: Vec3::from_array(scale),
            ..Transform::default()
        },
    )
}

/// Every visible mesh of `plate` and where it stands, without copying any of them.
///
/// The thumbnail is rendered from this rather than from `merge_plate`, because a render
/// reads each vertex once wherever it lives and merging would copy the whole plate for
/// nothing. Support meshes are already in plate coordinates.
pub fn plate_parts(scene: &Scene, plate: u32) -> Vec<(Arc<Mesh>, Transform)> {
    let mut parts = Vec::new();
    for object in scene.printable(plate) {
        let mesh = object.hollow.shell().unwrap_or(&object.mesh);
        parts.push((Arc::clone(mesh), object.transform));
        for supports in object.supports.meshes().unwrap_or_default() {
            parts.push((Arc::clone(supports), Transform::default()));
        }
    }
    parts
}

fn append(merged: &mut Mesh, mesh: &Mesh) {
    let offset = merged.vertices.len() as u32;
    merged.vertices.extend_from_slice(&mesh.vertices);
    merged.faces.extend(
        mesh.faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::ImportSummary;
    use core_geometry::{Bvh, Orientation, Scalar, Vec3, diagnose};
    use core_raster::{Grey, PixelPitch, RasterSettings, Rasterizer, ScanlineRasterizer, Shading};
    use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings, Winding};
    use core_volume::Shell;
    use printer_profiles::SupportProfile;
    use std::sync::Arc;

    fn merge_plate(scene: &Scene, plate: u32) -> Option<Mesh> {
        merge_plate_compensated(scene, plate, &Compensation::default())
    }

    /// Axis-aligned cube spanning 0..1 on every axis, twelve triangles.
    fn unit_cube() -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
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

    fn summary() -> ImportSummary {
        ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&unit_cube()),
        }
    }

    fn insert(scene: &mut Scene, transform: Transform) {
        scene.insert(crate::scene::Imported::new(
            "cube".to_owned(),
            Arc::new(unit_cube()),
            transform,
            summary(),
        ));
    }

    #[test]
    fn merging_one_plate_leaves_the_other_alone() {
        let mut scene = Scene::default();
        insert(&mut scene, Transform::default());
        insert(
            &mut scene,
            Transform::from_translation(Vec3::new(5.0, 0.0, 0.0)),
        );
        let second = scene.objects()[1].id;
        scene.add_plate();
        scene.move_to_plate(second, 1);

        let ground = merge_plate(&scene, 0).expect("the first plate has a cube");
        let other = merge_plate(&scene, 1).expect("the second plate has one too");
        assert_eq!(ground.faces.len(), 12, "one cube, not both");
        assert_eq!(other.faces.len(), 12);
        assert!(merge_plate(&scene, 2).is_none(), "there is no third plate");
    }

    #[test]
    fn an_empty_scene_has_nothing_to_slice() {
        assert!(merge_plate(&Scene::default(), 0).is_none());
    }

    #[test]
    fn two_objects_keep_all_their_faces_and_their_places() {
        let mut scene = Scene::default();
        insert(&mut scene, Transform::default());
        insert(
            &mut scene,
            Transform::from_translation(Vec3::new(5.0, 0.0, 0.0)),
        );

        let merged = merge_plate(&scene, 0).expect("two cubes have geometry");
        assert_eq!(merged.faces.len(), 24);
        assert_eq!(merged.vertices.len(), 16);

        let bounds = merged.aabb().expect("the merge has vertices");
        assert!(bounds.mins.abs_diff_eq(Vec3::ZERO, 1e-5));
        assert!(bounds.maxs.abs_diff_eq(Vec3::new(6.0, 1.0, 1.0), 1e-5));
    }

    #[test]
    fn the_second_objects_faces_point_at_its_own_vertices() {
        let mut scene = Scene::default();
        insert(&mut scene, Transform::default());
        insert(
            &mut scene,
            Transform::from_translation(Vec3::new(5.0, 0.0, 0.0)),
        );

        let merged = merge_plate(&scene, 0).expect("two cubes have geometry");
        for face in &merged.faces[12..] {
            assert!(
                face.iter().all(|index| *index >= 8),
                "the second cube must index its own vertices"
            );
        }
    }

    /// A cube ten millimetres across, hanging ten millimetres over the plate, with one
    /// support standing under the middle of it.
    fn supported_cube() -> Scene {
        let mut scene = Scene::default();
        let transform = Transform {
            translation: Vec3::new(0.0, 0.0, 10.0),
            scale: Vec3::splat(10.0),
            ..Transform::default()
        };
        insert(&mut scene, transform);

        let object = &mut scene.objects_mut()[0];
        object.supports.add(Vec3::new(5.0, 5.0, 10.0), transform, 0);
        let mesh = Arc::clone(&object.mesh);
        object.supports.refresh(
            &mesh,
            &Bvh::build(&mesh),
            transform,
            std::slice::from_ref(&SupportProfile::medium()),
        );
        scene
    }

    #[test]
    fn a_support_is_part_of_what_gets_sliced() {
        let scene = supported_cube();
        let standing = scene.objects()[0].supports.standing();
        assert_eq!(standing, 1, "the support found room under the overhang");

        let merged = merge_plate(&scene, 0).expect("the plate has geometry");
        assert!(
            merged.faces.len() > 12,
            "the merge must carry the column as well as the cube"
        );

        let bounds = merged.aabb().expect("the merge has vertices");
        assert!(
            bounds.mins.z <= 0.0,
            "the column reaches the plate, so the merge starts at z = 0, got {}",
            bounds.mins.z
        );
    }

    #[test]
    fn a_layer_under_the_overhang_exposes_the_column() {
        let merged = merge_plate(&supported_cube(), 0).expect("the plate has geometry");
        let settings = SliceSettings {
            layer_height: 0.05,
            ..SliceSettings::default()
        };
        let sliced = PlaneSliceEngine
            .slice(&merged, &settings)
            .expect("a closed mesh slices");

        // Five millimetres up is clear air under the cube, so the only thing on that
        // layer is the pillar: a ring of the profile's own diameter.
        let layer = sliced
            .layers
            .iter()
            .find(|layer| (layer.z - 5.0).abs() < settings.layer_height)
            .expect("the stack reaches the middle of the column");

        let area: f32 = layer.contours.iter().map(|contour| contour.area()).sum();
        let profile = SupportProfile::medium();
        let radius = profile.pillar_radius_mm();
        let expected = std::f32::consts::PI * radius * radius;
        assert!(
            (area - expected).abs() / expected < 0.05,
            "expected about {expected} mm2 of pillar, got {area}"
        );
    }

    /// A cube with a smaller cube inside it, wound inward: what `hollow` would hand back
    /// for a 0.25 mm wall, without building a field for it.
    fn shelled_cube() -> Mesh {
        let mut shell = unit_cube();
        let cavity = scaled_cube(0.5, 0.25);
        let offset = shell.vertices.len() as u32;
        shell.vertices.extend_from_slice(&cavity.vertices);
        shell.faces.extend(
            cavity
                .faces
                .iter()
                .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
        );
        shell
    }

    /// The unit cube shrunk by `size` and moved to `at` on every axis.
    fn scaled_cube(size: Scalar, at: Scalar) -> Mesh {
        let cube = unit_cube();
        Mesh::new(
            cube.vertices
                .iter()
                .map(|corner| *corner * size + Vec3::splat(at))
                .collect(),
            cube.faces,
        )
    }

    #[test]
    fn a_hollowed_model_is_sliced_as_its_shell() {
        let mut scene = Scene::default();
        insert(&mut scene, Transform::default());
        scene.objects_mut()[0].hollow.take(Shell {
            mesh: Arc::new(shelled_cube()),
            cavity_mm3: 0.125,
            voxel_mm: 0.2,
            coarsened: false,
            scale: Vec3::ONE,
            settings: core_volume::HollowSettings::default(),
        });

        let merged = merge_plate(&scene, 0).expect("the cube has geometry");
        assert_eq!(
            merged.faces.len(),
            24,
            "the shell goes in, not the solid the object was imported as"
        );

        let settings = SliceSettings {
            layer_height: 0.1,
            ..SliceSettings::default()
        };
        let sliced = PlaneSliceEngine
            .slice(&merged, &settings)
            .expect("the shell slices");
        let layer = sliced
            .layers
            .iter()
            .find(|layer| (layer.z - 0.5).abs() < settings.layer_height)
            .expect("the stack reaches the middle of the cube");

        // Halfway up, the cavity comes through as a contour wound the other way, which is
        // what the positive winding rule takes out of the square around it.
        assert_eq!(layer.contours.len(), 2);
        let area: Scalar = layer
            .contours
            .iter()
            .map(|contour| match contour.winding {
                Winding::Outer => contour.area(),
                Winding::Inner => -contour.area(),
            })
            .sum();
        assert!(
            (area - 0.75).abs() < 0.02,
            "a shelled unit cube exposes 1 - 0.5^2 = 0.75 mm2, got {area}"
        );
    }

    #[test]
    fn a_hidden_object_is_not_sliced() {
        let mut scene = Scene::default();
        insert(&mut scene, Transform::default());
        insert(&mut scene, Transform::default());
        scene.objects_mut()[1].visible = false;

        let merged = merge_plate(&scene, 0).expect("one cube is still visible");
        assert_eq!(merged.faces.len(), 12);
    }
    /// The whole window path for a drain hole: a hole is placed on a model, the model is
    /// hollowed with it, and the stack that goes to the printer has the hole in it.
    #[test]
    fn a_hole_placed_on_a_model_is_open_in_the_layers_that_cross_it() {
        let mut scene = Scene::default();
        let transform = Transform {
            scale: Vec3::splat(20.0),
            ..Transform::default()
        };
        insert(&mut scene, transform);

        let object = &mut scene.objects_mut()[0];
        let mesh = Arc::clone(&object.mesh);
        let bvh = Bvh::build(&mesh);
        // A click on the middle of the lid, which stands 20 mm up.
        object.hollow.add_drain(
            &mesh,
            &bvh,
            Vec3::new(10.0, 10.0, 20.0),
            Vec3::Z,
            crate::drain::DrainTool::default().size(),
            transform,
        );
        let settings = crate::hollow::HollowTool::default().settings();
        let asking = object.hollow.asking(&settings);
        let hollowed = core_volume::hollow(&mesh, &bvh, &asking).expect("a cube hollows");
        object.hollow.take(Shell {
            mesh: Arc::new(hollowed.mesh),
            cavity_mm3: hollowed.cavity_mm3,
            voxel_mm: hollowed.voxel_mm,
            coarsened: hollowed.coarsened,
            scale: transform.scale,
            settings: asking,
        });

        let merged = merge_plate(&scene, 0).expect("the plate has geometry");
        let sliced = PlaneSliceEngine
            .slice(
                &merged,
                &SliceSettings {
                    layer_height: 0.05,
                    ..SliceSettings::default()
                },
            )
            .expect("a closed mesh slices");
        // Half a millimetre under the lid: solid all the way across but for the hole.
        let layer = sliced
            .layers
            .iter()
            .find(|layer| (layer.z - 19.5).abs() < 0.05)
            .expect("the stack reaches the lid");

        let raster = RasterSettings {
            width_px: 400,
            height_px: 400,
            pitch: PixelPitch { x: 0.1, y: 0.1 },
            mirror_x: false,
            mirror_y: false,
            shading: Shading::Coverage,
            grey: Grey::default(),
            blur_px: 0,
        };
        let mask = ScanlineRasterizer
            .rasterize(layer, &raster)
            .expect("the layer rasterises")
            .runs
            .to_mask();
        // The lid is 20 mm square at the origin, and image rows grow the other way from
        // model Y, so its middle sits at row 300 of the 400 the panel is high.
        let lit = mask.pixels().iter().filter(|&&pixel| pixel > 0).count();
        assert_eq!(
            mask.pixels()[300 * 400 + 100],
            0x00,
            "the hole is open at the middle of the lid"
        );
        assert!(
            (40000 - lit) > 500,
            "a 3 mm hole takes about 700 px out of the 40000 the lid covers, got {}",
            40000 - lit
        );
    }

    #[test]
    fn a_shrinking_resin_prints_the_part_larger_where_it_already_stood() {
        let mut scene = Scene::default();
        insert(
            &mut scene,
            Transform::from_translation(Vec3::new(30.0, 40.0, 0.0)),
        );
        let plain =
            merge_plate_compensated(&scene, 0, &Compensation::default()).expect("a cube is there");
        let grown = merge_plate_compensated(
            &scene,
            0,
            &Compensation {
                shrink_x_pct: 101.0,
                shrink_y_pct: 101.0,
                ..Compensation::default()
            },
        )
        .expect("a cube is there");

        let before = plain.aabb().expect("a cube has bounds");
        let after = grown.aabb().expect("a cube has bounds");
        let width = |bounds: &core_geometry::Aabb| bounds.maxs.x - bounds.mins.x;
        let middle = |bounds: &core_geometry::Aabb| (bounds.mins.x + bounds.maxs.x) / 2.0;
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
}

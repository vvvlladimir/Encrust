use core_engine::Model;

use crate::scene::Scene;

/// Every visible model of `plate`, with its cavity, its cuts and its supports, as the
/// engine takes them.
///
/// A hollowed model goes in as its shell: the outer surface with the cavity wound the
/// other way, which is what the rasteriser's positive winding rule takes out; see
/// `docs/decisions/0059`. Nothing is copied — the meshes travel as the `Arc`s the scene
/// holds them in.
pub fn models_of(scene: &Scene, plate: u32) -> Vec<Model> {
    scene
        .printable(plate)
        .map(|object| Model {
            mesh: std::sync::Arc::clone(object.hollow.shell().unwrap_or(&object.mesh)),
            transform: object.transform,
            cuts: object.hollow.cut_bodies().cloned(),
            supports: object.supports.meshes().unwrap_or_default().to_vec(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::ImportSummary;
    use core_engine::bake;
    use core_geometry::{Bvh, Mesh, Orientation, Scalar, Transform, Vec3, diagnose};
    use core_raster::{Grey, PixelPitch, RasterSettings, Rasterizer, ScanlineRasterizer, Shading};
    use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings, Winding};
    use core_volume::Shell;
    use printer_profiles::{Compensation, SupportProfile};
    use std::sync::Arc;

    /// What the window hands the engine for `plate`, baked into one mesh.
    fn merge_plate(scene: &Scene, plate: u32) -> Option<Mesh> {
        bake(&models_of(scene, plate), &Compensation::default()).map(|baked| baked.mesh)
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
            cavity: 0..0,
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
    /// What a hole cuts is gone whatever else stands there: a lattice bonded into the wall,
    /// a support's tip, or here a second model laid over the first.
    #[test]
    fn a_hole_opens_through_every_body_that_overlaps_where_it_is_drilled() {
        let mut scene = Scene::default();
        let transform = Transform {
            scale: Vec3::splat(20.0),
            ..Transform::default()
        };
        insert(&mut scene, transform);
        insert(&mut scene, transform);

        let object = &mut scene.objects_mut()[0];
        let mesh = Arc::clone(&object.mesh);
        let bvh = Bvh::build(&mesh);
        object.hollow.add_drain(
            &mesh,
            &bvh,
            Vec3::new(10.0, 10.0, 20.0),
            Vec3::Z,
            crate::drain::DrainTool::default().size(),
            transform,
        );

        let merged = merge_plate(&scene, 0).expect("the plate has geometry");
        let sliced = PlaneSliceEngine
            .slice(
                &merged,
                &SliceSettings {
                    layer_height: 0.05,
                    ..SliceSettings::default()
                },
            )
            .expect("closed meshes slice");
        let layer = sliced
            .layers
            .iter()
            .find(|layer| (layer.z - 19.5).abs() < 0.05)
            .expect("the stack reaches the lid");
        let mask = ScanlineRasterizer
            .rasterize(layer, &lid_raster())
            .expect("the layer rasterises")
            .runs
            .to_mask();
        assert_eq!(
            mask.pixels()[LID_MIDDLE],
            0x00,
            "two cubes in one place count twice, and the hole still opens through both"
        );
    }

    /// The pixel under the middle of the lid on [`lid_raster`]: the lid fills the first 200
    /// rows and columns, so its middle is the hundredth of each.
    const LID_MIDDLE: usize = 100 * 400 + 100;

    /// A 40 mm square panel at a tenth of a millimetre, under the 20 mm lid of [`insert`].
    fn lid_raster() -> RasterSettings {
        RasterSettings {
            width_px: 400,
            height_px: 400,
            pitch: PixelPitch { x: 0.1, y: 0.1 },
            mirror_x: false,
            mirror_y: false,
            shading: Shading::Coverage,
            grey: Grey::default(),
            blur_px: 0,
        }
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
            cavity: hollowed.cavity,
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

        let mask = ScanlineRasterizer
            .rasterize(layer, &lid_raster())
            .expect("the layer rasterises")
            .runs
            .to_mask();
        let lit = mask.pixels().iter().filter(|&&pixel| pixel > 0).count();
        assert_eq!(
            mask.pixels()[LID_MIDDLE],
            0x00,
            "the hole is open at the middle of the lid"
        );
        assert!(
            (40000 - lit) > 500,
            "a 3 mm hole takes about 700 px out of the 40000 the lid covers, got {}",
            40000 - lit
        );
    }
}

//! The window's state as a project file, and back again.
//!
//! What goes in is the plate as it stands, geometry and all; what comes out is that plate
//! and nothing worked out anew (ADR 0191).

use std::sync::Arc;

use core_geometry::{Bvh, Vec3, center_of_mass};
use rayon::prelude::*;

use crate::cut::CutTool;
use crate::drain::DrainTool;

use crate::hollow::HollowTool;
use crate::scene::{ImportSummary, Imported, Scene, SceneObject};
use crate::slicing::Slicing;
use crate::supports::{SupportGroup, SupportTool};
use crate::workspace::Array;
use core_engine::project::{
    BuiltCavity, Cavity, Chosen, CutState, DrainState, Group, HollowState, Manifest, ModelMeshes,
    ObjectHollowState, ObjectState, ObjectSupportState, Project, SlicingState, Summary,
    SupportState, VERSION, hollow_of, supports_of,
};
use printer_profiles::SupportProfile;

/// What a project file is written from: the plate, and the numbers each tool is set to.
pub struct Captured<'a> {
    pub scene: &'a Scene,
    pub slicing: &'a Slicing,
    pub supports: &'a SupportTool,
    pub hollow: &'a HollowTool,
    pub drain: &'a DrainTool,
    pub cut: &'a CutTool,
    pub array: &'a Array,
}

/// The same, to be written into by a load.
pub struct CapturedMut<'a> {
    pub scene: &'a mut Scene,
    pub slicing: &'a mut Slicing,
    pub supports: &'a mut SupportTool,
    pub hollow: &'a mut HollowTool,
    pub drain: &'a mut DrainTool,
    pub cut: &'a mut CutTool,
    pub array: &'a mut Array,
}

/// The whole plate as a project, ready to be written.
pub fn capture(plate: Captured<'_>) -> Project {
    let Captured {
        scene,
        slicing,
        supports,
        hollow,
        drain,
        cut,
        array,
    } = plate;
    let manifest = Manifest {
        version: VERSION,
        printer: slicing.printer.clone().map(|profile| Chosen {
            id: slicing.printer_id.clone(),
            profile,
        }),
        resin: Some(Chosen {
            id: slicing.resin_id.clone(),
            profile: slicing.base_material().clone(),
        }),
        slicing: slicing_state(slicing),
        supports: support_state(supports),
        hollow: HollowState {
            thickness_mm: hollow.thickness_mm,
            mode: hollow.mode,
            precision: hollow.precision,
            infill_on: hollow.infill_on,
            infill: hollow.infill,
            blocker_mm: hollow.blocker_mm,
        },
        drain: DrainState {
            diameter_mm: drain.diameter_mm,
            depth_mm: drain.depth_mm,
            taper: drain.taper,
        },
        cut: CutState {
            axis: cut.axis,
            height_mm: cut.offset_mm,
            keep: cut.keep,
        },
        array: *array,
        plates: scene.plates().to_vec(),
        active_plate: scene.active_plate(),
        objects: scene.objects().iter().map(object_state).collect(),
    };
    Project {
        manifest,
        models: scene.objects().iter().map(model_meshes).collect(),
    }
}

/// The geometry one object is written with: what it was imported as, and the shell
/// standing on it.
fn model_meshes(object: &SceneObject) -> ModelMeshes {
    ModelMeshes {
        source: Arc::clone(&object.mesh),
        shell: object.hollow.shell().map(Arc::clone),
    }
}

fn slicing_state(slicing: &Slicing) -> SlicingState {
    SlicingState {
        layer_height_mm: slicing.layer_height_mm(),
        adaptive: slicing.adaptive,
        exposure: slicing.bands_as_measured(),
        samples: slicing.samples,
        anti_alias: slicing.anti_alias,
        grey_levels: slicing.grey_levels,
        blur_px: slicing.blur_px,
        remove_islands: slicing.remove_islands,
        format: slicing.format.into(),
    }
}

fn support_state(supports: &SupportTool) -> SupportState {
    SupportState {
        groups: supports
            .groups
            .iter()
            .zip(supports.table())
            .map(|(group, profile)| Group {
                name: group.name.clone(),
                profile,
            })
            .collect(),
        active: supports.active,
        fill: supports.fill,
        brush_radius_mm: supports.brush_radius_mm,
        flood_angle_deg: supports.flood_angle_deg,
    }
}

fn object_state(object: &SceneObject) -> ObjectState {
    ObjectState {
        name: object.name.clone(),
        plate: object.plate,
        transform: object.transform,
        visible: object.visible,
        summary: Summary {
            vertices_merged: object.summary.vertices_merged,
            faces_removed: object.summary.faces_removed,
            orientation: object.summary.orientation,
            diagnostics: object.summary.diagnostics.clone(),
        },
        supports: ObjectSupportState {
            points: object.supports.points().to_vec(),
            painted: object.supports.painted().clone(),
            blocked: object.supports.blocked().clone(),
            frozen: object.supports.frozen().to_vec(),
            grown: object.supports.grown(object.transform),
        },
        hollow: ObjectHollowState {
            blockers: object.hollow.blockers().to_vec(),
            drains: object.hollow.drains().to_vec(),
            channels: object.hollow.channels().to_vec(),
            built: built_cavity(object),
        },
    }
}

/// The shell standing on `object` as the file records it, or `None` for a solid model.
fn built_cavity(object: &SceneObject) -> Option<BuiltCavity> {
    let asked = object.hollow.asked()?;
    let (voxel_mm, coarsened) = object.hollow.lattice()?;
    Some(BuiltCavity {
        wall: Cavity::of(asked),
        cavity_faces: object.hollow.cavity_faces()?,
        cavity_mm3: object.hollow.cavity_mm3(),
        voxel_mm,
        coarsened,
        scale: object.transform.scale,
    })
}

/// Puts a loaded project in place of whatever the window was holding.
pub fn apply(project: Project, plate: CapturedMut<'_>) {
    let CapturedMut {
        scene,
        slicing,
        supports,
        hollow,
        drain,
        cut,
        array,
    } = plate;
    let Project { manifest, models } = project;

    if let Some(printer) = manifest.printer {
        slicing.set_printer(printer.profile, printer.id);
    }
    if let Some(resin) = manifest.resin {
        slicing.resin_id = resin.id;
        slicing.set_material(resin.profile);
    }
    restore_slicing(manifest.slicing, slicing);
    restore_supports(manifest.supports, supports);

    hollow.thickness_mm = manifest.hollow.thickness_mm;
    hollow.mode = manifest.hollow.mode;
    hollow.precision = manifest.hollow.precision;
    hollow.infill_on = manifest.hollow.infill_on;
    hollow.infill = manifest.hollow.infill;
    hollow.blocker_mm = manifest.hollow.blocker_mm;

    drain.diameter_mm = manifest.drain.diameter_mm;
    drain.depth_mm = manifest.drain.depth_mm;
    drain.taper = manifest.drain.taper;
    cut.axis = manifest.cut.axis;
    cut.offset_mm = manifest.cut.height_mm;
    cut.keep = manifest.cut.keep;
    *array = manifest.array;

    *scene = Scene::default();
    for (index, name) in manifest.plates.iter().enumerate() {
        if index > 0 {
            scene.add_plate();
        }
        scene.rename_plate(index as u32, name.clone());
    }
    restore_objects(scene, manifest.objects, models, &supports.table());
    scene.select(None);
    scene.show_plate(manifest.active_plate);
}

/// The bands are kept at the height the resin was measured at, so they go in before the
/// layer height carries them to the one the project was cut at.
fn restore_slicing(state: SlicingState, slicing: &mut Slicing) {
    slicing.exposure = state.exposure;
    slicing.set_layer_height(state.layer_height_mm);
    slicing.adaptive = state.adaptive;
    slicing.samples = state.samples;
    slicing.anti_alias = state.anti_alias;
    slicing.grey_levels = state.grey_levels;
    slicing.blur_px = state.blur_px;
    slicing.remove_islands = state.remove_islands;
    slicing.format = state.format.into();
}

fn restore_supports(state: SupportState, supports: &mut SupportTool) {
    if !state.groups.is_empty() {
        supports.groups = state
            .groups
            .into_iter()
            .map(|group| SupportGroup {
                name: group.name,
                profile: group.profile,
            })
            .collect();
        supports.active = state.active.min(supports.groups.len() as u16 - 1);
        supports.profile = supports.groups[supports.active as usize].profile.clone();
    }
    supports.fill = state.fill;
    supports.brush_radius_mm = state.brush_radius_mm;
    supports.flood_angle_deg = state.flood_angle_deg;
    supports.picked.clear();
}

/// Puts every object the file holds back on `scene`, with the shell, the cuts and the
/// trees it was saved with: nothing here is hollowed or grown again.
fn restore_objects(
    scene: &mut Scene,
    objects: Vec<ObjectState>,
    models: Vec<ModelMeshes>,
    table: &[SupportProfile],
) {
    // The hierarchy is the expensive half of opening a model and is per mesh, so the
    // plate's models are built at once rather than one after another.
    let measured: Vec<(Arc<Bvh>, Vec3)> = models
        .par_iter()
        .map(|model| {
            (
                Arc::new(Bvh::build(&model.source)),
                center_of_mass(&model.source).unwrap_or(Vec3::ZERO),
            )
        })
        .collect();

    for ((state, meshes), (bvh, center_of_mass)) in objects.into_iter().zip(&models).zip(measured) {
        let id = scene.insert(Imported {
            name: state.name,
            bvh: Arc::clone(&bvh),
            mesh: Arc::clone(&meshes.source),
            transform: state.transform,
            center_of_mass,
            summary: ImportSummary {
                vertices_merged: state.summary.vertices_merged,
                faces_removed: state.summary.faces_removed,
                orientation: state.summary.orientation,
                diagnostics: state.summary.diagnostics,
            },
            // A project carries meshes, not the files they came from: a texture that was
            // not pressed in before saving is not in there to press in now.
            mapped: None,
        });
        let Some(object) = scene.get_mut(id) else {
            continue;
        };
        object.visible = state.visible;
        object.plate = state.plate;
        object.supports = supports_of(
            &state.supports,
            &meshes.source,
            &bvh,
            state.transform,
            table,
        );
        object.hollow = hollow_of(&state.hollow, meshes, bvh);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use core_geometry::{Mesh, Quat, Transform, Vec3, diagnose};
    use core_supports::{ModelSupports, SupportPoint};
    use core_volume::{Blocker, Channel, HollowMode, ModelHollow};

    use crate::cut::Keep;

    use super::*;
    use crate::scene::ImportSummary;
    use core_engine::project::{read_from, write_to};

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
            orientation: core_geometry::Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&unit_cube()),
        }
    }

    /// A plate with one model on it carrying something from every tool the file records.
    #[allow(
        clippy::field_reassign_with_default,
        reason = "Slicing keeps a private field"
    )]
    struct Bench {
        scene: Scene,
        slicing: Slicing,
        supports: SupportTool,
        hollow: HollowTool,
        drain: DrainTool,
        cut: CutTool,
        array: Array,
    }

    impl Bench {
        fn plate(&self) -> Captured<'_> {
            Captured {
                scene: &self.scene,
                slicing: &self.slicing,
                supports: &self.supports,
                hollow: &self.hollow,
                drain: &self.drain,
                cut: &self.cut,
                array: &self.array,
            }
        }

        fn plate_mut(&mut self) -> CapturedMut<'_> {
            CapturedMut {
                scene: &mut self.scene,
                slicing: &mut self.slicing,
                supports: &mut self.supports,
                hollow: &mut self.hollow,
                drain: &mut self.drain,
                cut: &mut self.cut,
                array: &mut self.array,
            }
        }
    }

    fn bench() -> Bench {
        let mesh = Arc::new(unit_cube());
        let mut scene = Scene::default();
        let id = scene.insert(Imported::new(
            "cube".to_owned(),
            Arc::clone(&mesh),
            Transform {
                translation: Vec3::new(12.0, 34.0, 0.0),
                rotation: Quat::from_rotation_z(0.5),
                scale: Vec3::splat(2.0),
            },
            ImportSummary {
                vertices_merged: 3,
                faces_removed: 1,
                orientation: core_geometry::Orientation {
                    flipped_faces: 4,
                    inverted_shells: 1,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));

        let object = scene.get_mut(id).expect("the cube was just inserted");
        object.visible = false;
        let mut painted = core_supports::Region::default();
        painted.set(5, true);
        painted.set(9, true);
        let mut blocked = core_supports::Region::default();
        blocked.set(2, true);
        object.supports = ModelSupports::restore(
            vec![
                SupportPoint::new(Vec3::new(0.5, 0.5, 1.0)),
                SupportPoint::new(Vec3::new(0.2, 0.7, 1.0)).in_group(1),
            ],
            painted,
            blocked,
            Vec::new(),
        );
        object.hollow = ModelHollow::restored(
            Arc::clone(&object.mesh),
            Arc::clone(&object.bvh),
            vec![Blocker::ball(Vec3::splat(0.5), 0.2)],
            Vec::new(),
            vec![Channel {
                points: vec![Vec3::ZERO, Vec3::Z],
                diameter_mm: 1.5,
            }],
        );

        let mut slicing = Slicing::default();
        slicing.set_layer_height(0.035);
        slicing.anti_alias = false;
        slicing.grey_levels = std::num::NonZeroU8::new(4);
        slicing.blur_px = 2;
        slicing.remove_islands = true;
        slicing.samples = std::num::NonZeroU8::new(3).expect("three is not zero");
        slicing.exposure = vec![core_format::ExposureRange::new(0.0, 4.0, 9.5)];
        slicing.adaptive = Some(core_slicer::AdaptiveSettings::default());

        let mut supports = SupportTool {
            brush_radius_mm: 4.5,
            flood_angle_deg: 25.0,
            ..SupportTool::default()
        };
        supports.add_group();

        let hollow = HollowTool {
            thickness_mm: 1.75,
            mode: HollowMode::External,
            infill_on: true,
            ..HollowTool::default()
        };

        Bench {
            scene,
            slicing,
            supports,
            hollow,
            drain: DrainTool {
                diameter_mm: 4.25,
                depth_mm: 6.0,
                taper: 0.5,
                ..DrainTool::default()
            },
            cut: CutTool {
                axis: crate::scene::Axis::X,
                offset_mm: 17.5,
                keep: Keep::Above,
            },
            array: Array {
                columns: 3,
                rows: 4,
                gap_mm: 7.5,
            },
        }
    }

    /// Writes a plate and reads it back into an empty window.
    fn round_trip(saved: &Bench) -> Bench {
        let project = capture(saved.plate());
        let mut bytes = Cursor::new(Vec::new());
        write_to(&mut bytes, &project).expect("a project in memory writes");
        bytes.set_position(0);
        let read = read_from(bytes).expect("what was just written reads");

        let mut back = Bench {
            scene: Scene::default(),
            slicing: Slicing::default(),
            supports: SupportTool::default(),
            hollow: HollowTool::default(),
            drain: DrainTool::default(),
            cut: CutTool::default(),
            array: Array::default(),
        };
        apply(read, back.plate_mut());
        back
    }

    #[test]
    fn a_saved_plate_opens_as_the_plate_that_was_saved() {
        let saved = bench();
        let back = round_trip(&saved);

        assert_eq!(back.scene.objects().len(), 1);
        let before = &saved.scene.objects()[0];
        let after = &back.scene.objects()[0];
        assert_eq!(after.name, before.name);
        assert_eq!(after.transform, before.transform);
        assert_eq!(after.visible, before.visible, "a hidden model stays hidden");
        assert_eq!(*after.mesh, *before.mesh, "a face index is what a patch is");
        assert_eq!(after.summary.vertices_merged, 3, "repair is not re-run");
        assert_eq!(after.supports.points(), before.supports.points());
        assert_eq!(after.supports.painted(), before.supports.painted());
        assert_eq!(after.supports.blocked(), before.supports.blocked());
        assert_eq!(after.hollow.blockers(), before.hollow.blockers());
        assert_eq!(after.hollow.channels(), before.hollow.channels());
    }

    #[test]
    fn every_tool_opens_set_the_way_it_was_saved() {
        let saved = bench();
        let back = round_trip(&saved);

        assert_eq!(
            back.slicing.layer_height_mm(),
            saved.slicing.layer_height_mm()
        );
        assert_eq!(back.slicing.anti_alias, saved.slicing.anti_alias);
        assert_eq!(back.slicing.grey_levels, saved.slicing.grey_levels);
        assert_eq!(back.slicing.blur_px, saved.slicing.blur_px);
        assert_eq!(back.slicing.remove_islands, saved.slicing.remove_islands);
        assert_eq!(back.slicing.samples, saved.slicing.samples);
        assert_eq!(back.slicing.exposure, saved.slicing.exposure);
        assert_eq!(back.slicing.adaptive, saved.slicing.adaptive);
        assert_eq!(back.supports.groups.len(), saved.supports.groups.len());
        assert_eq!(back.supports.active, saved.supports.active);
        assert_eq!(
            back.supports.brush_radius_mm,
            saved.supports.brush_radius_mm
        );
        assert_eq!(
            back.supports.flood_angle_deg,
            saved.supports.flood_angle_deg
        );
        assert_eq!(back.hollow.thickness_mm, saved.hollow.thickness_mm);
        assert_eq!(back.hollow.mode, saved.hollow.mode);
        assert!(back.hollow.infill_on);
        assert_eq!(back.drain.diameter_mm, saved.drain.diameter_mm);
        assert_eq!(back.drain.depth_mm, saved.drain.depth_mm);
        assert_eq!(back.drain.taper, saved.drain.taper);
        assert_eq!(back.cut.axis, saved.cut.axis);
        assert_eq!(back.cut.offset_mm, saved.cut.offset_mm);
        assert_eq!(back.cut.keep, saved.cut.keep);
        assert_eq!(back.array, saved.array);
    }

    #[test]
    fn the_plates_and_what_stands_on_each_survive_a_round_trip() {
        let mut saved = bench();
        saved.scene.add_plate();
        saved.scene.rename_plate(1, "Small parts".to_owned());
        saved.scene.insert(Imported::new(
            "second".to_owned(),
            Arc::new(unit_cube()),
            Transform::default(),
            summary(),
        ));
        saved.scene.show_plate(1);

        let back = round_trip(&saved);

        assert_eq!(back.scene.plates(), ["Plate 1", "Small parts"]);
        assert_eq!(
            back.scene.active_plate(),
            1,
            "it opens on the plate it was left on"
        );
        assert_eq!(back.scene.here().count(), 1);
        let plates: Vec<u32> = back
            .scene
            .objects()
            .iter()
            .map(|object| object.plate)
            .collect();
        assert_eq!(plates, [0, 1]);
        let names: Vec<&str> = back
            .scene
            .objects()
            .iter()
            .map(|object| object.name.as_str())
            .collect();
        assert_eq!(names, ["cube", "second"]);
    }

    #[test]
    fn moving_a_model_changes_what_the_close_guard_compares() {
        let mut saved = bench();
        let before = crate::project::digest(&capture(saved.plate()).manifest);
        assert_eq!(
            before,
            crate::project::digest(&capture(saved.plate()).manifest),
            "the same plate has to hash the same, or every close would ask"
        );

        saved.scene.objects_mut()[0].transform.translation.x += 1.0;
        assert_ne!(
            before,
            crate::project::digest(&capture(saved.plate()).manifest),
            "a model that moved is work nobody has written down"
        );
    }

    /// A shell as a run would leave it on the bench's only model: its own mesh, which no
    /// hollowing would produce, so a rebuilt cavity could not be mistaken for this one.
    fn hollowed(saved: &mut Bench) -> Arc<Mesh> {
        let asked = saved.hollow.settings();
        let object = &mut saved.scene.objects_mut()[0];
        let settings = object.hollow.asking(&asked);
        let shell = Arc::new(unit_cube());
        object.hollow.take(core_volume::Shell {
            mesh: Arc::clone(&shell),
            cavity: 0..2,
            cavity_mm3: 1.5,
            voxel_mm: 0.1,
            coarsened: true,
            scale: object.transform.scale,
            settings,
        });
        shell
    }

    #[test]
    fn an_opened_plate_stands_as_it_was_saved_and_builds_nothing() {
        let mut saved = bench();
        let shell = hollowed(&mut saved);

        let back = round_trip(&saved);
        let object = &back.scene.objects()[0];
        assert_eq!(
            object.hollow.shell().map(|mesh| &**mesh),
            Some(&*shell),
            "the shell is read out of the file"
        );
        assert_eq!(object.hollow.cavity_faces(), Some(0..2));
        assert_eq!(object.hollow.lattice(), Some((0.1, true)));
        assert!(back.hollow.job.is_none(), "nothing is hollowed on opening");
    }

    #[test]
    fn the_wall_a_model_was_hollowed_to_is_what_the_panel_reads_back() {
        let mut saved = bench();
        let asked = saved.hollow.settings();
        hollowed(&mut saved);

        let project = capture(saved.plate());
        let built = project.manifest.objects[0]
            .hollow
            .built
            .as_ref()
            .expect("the model was hollowed");
        assert_eq!(built.wall, Cavity::of(&asked));

        let back = round_trip(&saved);
        assert!(
            !back.scene.objects()[0]
                .hollow
                .is_stale(&back.hollow.settings(), back.scene.objects()[0].transform),
            "a plate opened as it was saved is not stale"
        );
    }

    #[test]
    fn the_trees_a_run_grew_stand_again_without_being_grown() {
        let mut saved = bench();
        let table = saved.supports.table();
        let object = &mut saved.scene.objects_mut()[0];
        // Lifted off the plate, so the underside has room for a column to stand in.
        object.transform.translation.z = 5.0;
        let (mesh, bvh, transform) = (
            Arc::clone(&object.mesh),
            Arc::clone(&object.bvh),
            object.transform,
        );
        let under = transform
            .to_matrix()
            .transform_point3(Vec3::new(0.5, 0.5, 0.0));
        object.supports.add(under, transform, 0);
        object.supports.refresh(&mesh, &bvh, transform, &table);
        let standing = object.supports.trees().to_vec();
        assert!(!standing.is_empty(), "the points grew into trees");

        let back = round_trip(&saved);
        let object = &back.scene.objects()[0];
        assert_eq!(
            object.supports.trees(),
            standing,
            "every tree is the one saved"
        );
        assert!(
            object
                .supports
                .meshes()
                .is_some_and(|groups| groups.iter().any(|mesh| !mesh.is_empty())),
            "and it is meshed to be seen and sliced"
        );
    }
}

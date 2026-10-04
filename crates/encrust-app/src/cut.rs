use core_geometry::{Mesh, Plane, Scalar, Transform, Vec3, cut, diagnose, split, transform_mesh};

pub use core_engine::project::Keep;

use core_engine::project::Axis;

use crate::scene::{ImportSummary, Imported, ObjectId, Scene, SceneObject};
use crate::status::Status;

/// Where the Cut tool's plane is, and what it does with the halves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutTool {
    pub axis: Axis,
    /// Where the plane crosses the axis, millimetres: across Z, the height above the plate;
    /// across X or Y, the distance from the model's centre of mass.
    pub offset_mm: Scalar,
    pub keep: Keep,
}

impl Default for CutTool {
    fn default() -> Self {
        Self {
            axis: Axis::Z,
            offset_mm: 10.0,
            keep: Keep::Both,
        }
    }
}

impl CutTool {
    /// The plane the tool is set to for one model, in plate coordinates.
    pub fn plane(&self, object: &SceneObject) -> Option<Plane> {
        Plane::at(normal(self.axis), self.position_mm(object))
    }

    /// Where the plane crosses its axis for one model, in plate millimetres.
    pub fn position_mm(&self, object: &SceneObject) -> Scalar {
        match self.axis {
            Axis::X => object.pivot().translation.x + self.offset_mm,
            Axis::Y => object.pivot().translation.y + self.offset_mm,
            Axis::Z => self.offset_mm,
        }
    }

    /// Turns the plane to `axis`, through the model's centre of mass on every axis: the
    /// offset meant for one axis means nothing on another.
    pub fn set_axis(&mut self, axis: Axis, object: &SceneObject) {
        self.axis = axis;
        self.offset_mm = match axis {
            Axis::X | Axis::Y => 0.0,
            Axis::Z => object.pivot().translation.z,
        };
    }

    /// Cuts one model in two, in plate coordinates, and puts the halves on the plate in
    /// its place.
    ///
    /// What the model carried — its supports, its cavity, its drain holes — does not
    /// survive: the halves are new geometry, and a support placed on a surface that has
    /// been cut away has nothing to hold.
    pub fn apply(&self, scene: &mut Scene, id: ObjectId) -> Status {
        let Some(object) = scene.get(id) else {
            return Status::Error("nothing is selected to cut".to_owned());
        };
        let Some(plane) = self.plane(object) else {
            return Status::Error("the cut plane has no direction".to_owned());
        };
        let position_mm = self.position_mm(object);

        let name = object.name.clone();
        let mesh = object.hollow.shell().unwrap_or(&object.mesh);
        let placed = transform_mesh(mesh, object.transform);
        let halves = cut(&placed, plane);
        // A plane that misses leaves the model whole. Replacing it with a copy of itself
        // would throw away its supports and its cavity for nothing.
        if halves.below.is_empty() || halves.above.is_empty() {
            return Status::Error(format!(
                "the plane misses {name}, so there is nothing to cut"
            ));
        }

        let pieces: Vec<(String, Mesh)> = match self.keep {
            Keep::Both => vec![
                (format!("{name} (lower)"), halves.below),
                (format!("{name} (upper)"), halves.above),
            ],
            Keep::Below => vec![(name.clone(), halves.below)],
            Keep::Above => vec![(name.clone(), halves.above)],
        };
        scene.remove(id);
        for (name, mesh) in pieces {
            place(scene, name, mesh);
        }
        match halves.open_loops {
            0 => Status::Info(format!(
                "Cut {name} at {} {position_mm:.1} mm",
                self.axis.label()
            )),
            open => Status::Info(format!(
                "Cut {name}; {open} cut loops were open and left uncapped"
            )),
        }
    }
}

/// Breaks one model into the pieces that share no vertex, and puts them on the plate in
/// its place. A model that is all one piece is left alone.
pub fn split_parts(scene: &mut Scene, id: ObjectId) -> Status {
    let Some(object) = scene.get(id) else {
        return Status::Error("nothing is selected to split".to_owned());
    };
    let name = object.name.clone();
    let mesh = object.hollow.shell().unwrap_or(&object.mesh);
    let placed = transform_mesh(mesh, object.transform);

    let parts = split(&placed);
    if parts.len() < 2 {
        return Status::Info(format!("{name} is one piece"));
    }

    let count = parts.len();
    scene.remove(id);
    for (index, part) in parts.into_iter().enumerate() {
        place(scene, format!("{name} (part {})", index + 1), part);
    }
    Status::Info(format!("Split {name} into {count} parts"))
}

/// Puts a mesh that came out of a cut on the plate. It is already in plate coordinates,
/// so it goes down with no placement of its own.
fn place(scene: &mut Scene, name: String, mesh: Mesh) {
    let summary = ImportSummary {
        vertices_merged: 0,
        faces_removed: 0,
        orientation: core_geometry::Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        },
        diagnostics: diagnose(&mesh),
    };
    scene.insert(Imported::new(
        name,
        std::sync::Arc::new(mesh),
        Transform::default(),
        summary,
    ));
}

fn normal(axis: Axis) -> Vec3 {
    match axis {
        Axis::X => Vec3::X,
        Axis::Y => Vec3::Y,
        Axis::Z => Vec3::Z,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::signed_volume;
    use std::sync::Arc;

    /// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
    fn cuboid(size: Vec3) -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(size.x, 0.0, 0.0),
            Vec3::new(size.x, size.y, 0.0),
            Vec3::new(0.0, size.y, 0.0),
            Vec3::new(0.0, 0.0, size.z),
            Vec3::new(size.x, 0.0, size.z),
            Vec3::new(size.x, size.y, size.z),
            Vec3::new(0.0, size.y, size.z),
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

    fn scene_with(mesh: Mesh, transform: Transform) -> (Scene, ObjectId) {
        let mesh = Arc::new(mesh);
        let mut scene = Scene::default();
        let id = scene.insert(Imported::new(
            "cube".to_owned(),
            Arc::clone(&mesh),
            transform,
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: core_geometry::Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));
        (scene, id)
    }

    #[test]
    fn a_cut_leaves_two_models_where_there_was_one() {
        let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), Transform::default());
        let tool = CutTool {
            offset_mm: 4.0,
            ..CutTool::default()
        };

        let status = tool.apply(&mut scene, id);
        assert!(!status.is_error());
        assert_eq!(scene.objects().len(), 2);

        let volumes: Vec<f32> = scene
            .objects()
            .iter()
            .map(|object| signed_volume(&object.mesh))
            .collect();
        assert!(volumes.iter().any(|volume| (volume - 400.0).abs() < 1e-1));
        assert!(volumes.iter().any(|volume| (volume - 600.0).abs() < 1e-1));
    }

    #[test]
    fn the_cut_is_made_where_the_model_stands() {
        // The cube is lifted 20 mm, so a plane at 4 mm passes under it entirely.
        let lifted = Transform::from_translation(Vec3::new(0.0, 0.0, 20.0));
        let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), lifted);
        let tool = CutTool {
            offset_mm: 4.0,
            ..CutTool::default()
        };

        let status = tool.apply(&mut scene, id);
        assert!(
            status.is_error(),
            "a plane that misses the model cuts nothing"
        );
        assert_eq!(scene.objects().len(), 1, "and the model is left alone");
    }

    #[test]
    fn keeping_one_half_throws_the_other_away() {
        let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), Transform::default());
        let tool = CutTool {
            offset_mm: 6.0,
            keep: Keep::Below,
            ..CutTool::default()
        };

        tool.apply(&mut scene, id);
        assert_eq!(scene.objects().len(), 1);
        let volume = signed_volume(&scene.objects()[0].mesh);
        assert!((volume - 600.0).abs() < 1e-1, "got {volume}");
    }

    #[test]
    fn cutting_across_the_other_axes_is_the_same_cut() {
        // The cube's centre of mass is at 5 mm, so 2 mm short of it is 3 mm into the cube.
        for (axis, expected) in [(Axis::X, 300.0), (Axis::Y, 300.0)] {
            let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), Transform::default());
            let tool = CutTool {
                axis,
                offset_mm: -2.0,
                keep: Keep::Below,
            };
            tool.apply(&mut scene, id);

            let volume = signed_volume(&scene.objects()[0].mesh);
            assert!(
                (volume - expected).abs() < 1e-1,
                "cutting on {} gave {volume}",
                axis.label()
            );
        }
    }

    #[test]
    fn a_cut_across_x_follows_the_model_across_the_plate() {
        let moved = Transform::from_translation(Vec3::new(40.0, 25.0, 0.0));
        let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), moved);
        let tool = CutTool {
            axis: Axis::X,
            offset_mm: 0.0,
            keep: Keep::Below,
        };

        let status = tool.apply(&mut scene, id);
        assert!(
            !status.is_error(),
            "a plane through the centre of mass meets the model"
        );
        let volume = signed_volume(&scene.objects()[0].mesh);
        assert!(
            (volume - 500.0).abs() < 1e-1,
            "through the middle of a 10 mm cube leaves half of 1000 mm³, got {volume}"
        );
    }

    #[test]
    fn turning_the_plane_puts_it_through_the_centre_of_mass() {
        let lifted = Transform::from_translation(Vec3::new(0.0, 0.0, 20.0));
        let (scene, id) = scene_with(cuboid(Vec3::splat(10.0)), lifted);
        let object = scene.get(id).expect("the cube was just inserted");
        let mut tool = CutTool::default();

        tool.set_axis(Axis::Z, object);
        assert!(
            (tool.position_mm(object) - 25.0).abs() < 1e-4,
            "20 mm up, 5 mm in"
        );
        tool.set_axis(Axis::Y, object);
        assert!(
            (tool.position_mm(object) - 5.0).abs() < 1e-4,
            "the cube spans 0..10 in Y"
        );
    }

    #[test]
    fn splitting_a_cut_model_gives_a_part_each() {
        let mut both = cuboid(Vec3::splat(4.0));
        let away = cuboid(Vec3::splat(4.0));
        let offset = both.vertices.len() as u32;
        both.vertices
            .extend(away.vertices.iter().map(|v| *v + Vec3::new(20.0, 0.0, 0.0)));
        both.faces.extend(
            away.faces
                .iter()
                .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
        );

        let (mut scene, id) = scene_with(both, Transform::default());
        let status = split_parts(&mut scene, id);
        assert!(!status.is_error());
        assert_eq!(scene.objects().len(), 2);
    }

    #[test]
    fn splitting_one_solid_leaves_it_alone() {
        let (mut scene, id) = scene_with(cuboid(Vec3::splat(4.0)), Transform::default());
        let status = split_parts(&mut scene, id);
        assert!(!status.is_error());
        assert_eq!(scene.objects().len(), 1);
        assert_eq!(scene.objects()[0].id, id, "the model is the same one");
    }
}

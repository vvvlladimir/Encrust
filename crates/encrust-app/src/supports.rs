use anyhow::{Result, bail};
use core_geometry::{Adjacency, Scalar, Vec2, Vec3, lift_over_plate};
use core_supports::{Part, Placed, ProjectSettings, project};
use printer_profiles::SupportProfile;

use crate::job::{SupportJob, SupportOutcome, SupportRequest, tasks_of};
use crate::scene::Scene;
use crate::status::Status;

/// What a click on the model does while the Supports tool is in hand: stand one support,
/// paint the patch a fill covers, or paint the patch supports keep out of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placing {
    #[default]
    Point,
    Paint,
    Block,
}

impl Placing {
    pub const ALL: [Self; 3] = [Self::Point, Self::Paint, Self::Block];

    pub fn label(self) -> &'static str {
        match self {
            Self::Point => "Place",
            Self::Paint => "Paint",
            Self::Block => "Block",
        }
    }

    /// Whether this paints a patch rather than working on a single support.
    pub fn paints(self) -> bool {
        matches!(self, Self::Paint | Self::Block)
    }

    /// Whether what it paints is the patch supports keep out of.
    pub fn blocks(self) -> bool {
        self == Self::Block
    }
}

/// A named set of supports built to one profile. A support carries the number of its
/// group, so retuning a group retunes every support in it; see `docs/decisions/0094`.
#[derive(Debug, Clone)]
pub struct SupportGroup {
    pub name: String,
    pub profile: SupportProfile,
}

/// Which profile the drawing of a support edits: the group in the tool's hands, or the
/// one open on the Settings screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Editing {
    Tool,
    Settings,
}

/// The window a support's measurements are set on over a drawing of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parameters {
    pub editing: Editing,
    /// The thin model-to-model strut is drawn rather than a regular support.
    pub small_pillar: bool,
}

impl Parameters {
    pub fn new(editing: Editing) -> Self {
        Self {
            editing,
            small_pillar: false,
        }
    }
}

/// One piece of one frozen support: what the Edit mode selects, shows picked and carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Picked {
    pub id: crate::scene::ObjectId,
    /// Which of the object's frozen trees, which is stable until it is taken away.
    pub frozen: usize,
    pub part: Part,
}

/// What the Supports tool places with, and the automatic run it has going.
#[derive(Debug)]
pub struct SupportTool {
    /// The active group's numbers, which the panel's fields edit in place. They are
    /// written back into `groups` whenever another group is chosen.
    pub profile: SupportProfile,
    /// Every group on the plate, the first of which everything falls back to.
    pub groups: Vec<SupportGroup>,
    /// Which group new supports join, and whose numbers `profile` is holding.
    pub active: u16,
    /// The parts the Edit mode is working on. Several at once: a drag carries all of
    /// them by the same step.
    pub picked: Vec<Picked>,
    /// Where the part under the cursor stood when the drag began, in plate coordinates,
    /// so that every step is measured from the same place.
    pub anchor: Option<Vec3>,
    pub placing: Placing,
    /// Radius of the brush, millimetres of plate.
    pub brush_radius_mm: Scalar,
    /// How far a surface may turn from the face that was clicked and still be painted
    /// with it, degrees.
    pub flood_angle_deg: Scalar,
    pub fill: ProjectSettings,
    /// The brush has hold of the pointer: the press that started this drag landed on a
    /// model, so the stroke paints instead of turning the camera.
    pub stroke: bool,
    /// Where the brush was last put down, in panel points, so that a fast drag is painted
    /// as the line it drew rather than as the frames it was sampled on.
    pub last_daub: Option<Vec2>,
    pub job: Option<SupportJob>,
    /// The drawing of a support, while it is open.
    pub parameters: Option<Parameters>,
    /// The group the panel is asking about before it takes it away, and the supports it
    /// holds, or `None` when it is asking nothing.
    pub dropping: Option<(u16, usize)>,
}

impl Default for SupportTool {
    fn default() -> Self {
        Self {
            profile: SupportProfile::default(),
            groups: vec![SupportGroup {
                name: "Default".to_owned(),
                profile: SupportProfile::default(),
            }],
            active: 0,
            picked: Vec::new(),
            anchor: None,
            placing: Placing::default(),
            brush_radius_mm: 2.0,
            flood_angle_deg: 30.0,
            fill: ProjectSettings::default(),
            stroke: false,
            last_daub: None,
            job: None,
            parameters: None,
            dropping: None,
        }
    }
}

impl SupportTool {
    /// Why the Generate button is greyed out, or `None` when it is not.
    pub fn blocker(&self, scene: &Scene) -> Option<&'static str> {
        if scene.target_count() == 0 {
            return Some("Nothing visible on the plate to hold up.");
        }
        None
    }

    /// Starts an automatic placement run over everything visible on the plate.
    ///
    /// The run cuts each model at the height it will be printed at, so the layers it
    /// looks at are the layers the printer will expose.
    pub fn start(&mut self, scene: &Scene, layer_height_mm: Scalar) -> Result<()> {
        let tasks = tasks_of(scene);
        if tasks.is_empty() {
            bail!("nothing visible on the plate to hold up");
        }

        self.job = Some(SupportJob::spawn(SupportRequest {
            tasks,
            layer_height_mm,
            profile: self.profile.clone(),
        }));
        Ok(())
    }

    /// Drains a running placement run into the scene and the stage notice. Returns whether
    /// one is still going, which is what tells the window to keep repainting.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let Some(job) = self.job.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return true;
        };

        self.job = None;
        let group = self.active;
        let added = outcome.count();
        match outcome {
            SupportOutcome::Placed(placements) => {
                for placement in placements {
                    let Some(object) = scene.get_mut(placement.id) else {
                        continue;
                    };
                    for contact in placement.contacts {
                        object.supports.add(contact, placement.transform, group);
                    }
                }
                *status = Status::Info(match added {
                    0 => "Nothing on the plate needs holding up".to_owned(),
                    1 => "Placed 1 support".to_owned(),
                    added => format!("Placed {added} supports"),
                });
            }
            SupportOutcome::Cancelled => *status = Status::Info("Placement cancelled".to_owned()),
            SupportOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }

    /// Every group's profile, with the active group's taken from the numbers being
    /// edited rather than from the stale copy in the table.
    pub fn table(&self) -> Vec<SupportProfile> {
        self.groups
            .iter()
            .enumerate()
            .map(|(group, entry)| {
                if group == self.active as usize {
                    self.profile.clone()
                } else {
                    entry.profile.clone()
                }
            })
            .collect()
    }

    /// Makes `group` the one new supports join and the one the panel's fields edit,
    /// keeping what was edited of the group being left.
    pub fn choose_group(&mut self, group: u16) {
        if group as usize >= self.groups.len() || group == self.active {
            return;
        }
        self.groups[self.active as usize].profile = self.profile.clone();
        self.active = group;
        self.profile = self.groups[group as usize].profile.clone();
    }

    /// Adds a group starting from the numbers in hand and makes it the active one.
    pub fn add_group(&mut self) {
        self.groups[self.active as usize].profile = self.profile.clone();
        self.groups.push(SupportGroup {
            name: format!("Group {}", self.groups.len() + 1),
            profile: self.profile.clone(),
        });
        self.active = (self.groups.len() - 1) as u16;
    }

    /// How many supports stand in `group`, which is what dropping it would take away.
    pub fn group_holds(&self, group: u16, scene: &Scene) -> usize {
        scene
            .objects()
            .iter()
            .map(|object| object.supports.group_count(group))
            .sum()
    }

    /// Takes a group away, and with it every support built to it. The default group
    /// itself cannot go: something has to hold the raft's numbers.
    ///
    /// Returns how many supports went. A group is a shape, so its supports leave with it
    /// rather than being rebuilt to another group's numbers; `Cmd+Z` puts them back.
    pub fn remove_group(&mut self, group: u16, scene: &mut Scene) -> usize {
        if group == 0 || group as usize >= self.groups.len() {
            return 0;
        }
        if self.active != group {
            self.groups[self.active as usize].profile = self.profile.clone();
        }
        self.groups.remove(group as usize);
        let gone = scene
            .objects_mut()
            .iter_mut()
            .map(|object| object.supports.drop_group(group))
            .sum();
        self.active = 0;
        self.profile = self.groups[0].profile.clone();
        gone
    }

    /// Fills every painted patch on the plate with supports, and returns how many were
    /// put down.
    ///
    /// The face adjacency the rim is walked over is built here and thrown away: a fill is
    /// one press of a button, and keeping it beside every model would cost the memory of
    /// a third mesh for something a plate full of models asks for once.
    pub fn fill(&self, scene: &mut Scene) -> usize {
        let filled: Vec<(crate::scene::ObjectId, Vec<Vec3>)> = scene
            .targets()
            .filter(|object| !object.supports.painted().is_empty())
            .map(|object| {
                let keep_out = object.supports.keep_out(&object.mesh, object.transform);
                let placed = Placed::new(&object.mesh, &object.bvh, object.transform)
                    .blocking(keep_out.as_ref());
                let adjacency = Adjacency::of(&object.mesh);
                let matrix = object.transform.to_matrix();
                let standing: Vec<Vec3> = object
                    .supports
                    .points()
                    .iter()
                    .map(|point| matrix.transform_point3(point.contact))
                    .collect();
                (
                    object.id,
                    project(
                        &placed,
                        &adjacency,
                        object.supports.painted(),
                        &self.fill,
                        &standing,
                        self.profile.max_overhang_deg,
                    ),
                )
            })
            .collect();

        let mut added = 0;
        for (id, contacts) in filled {
            let Some(object) = scene.get_mut(id) else {
                continue;
            };
            let transform = object.transform;
            for contact in contacts {
                object.supports.add(contact, transform, self.active);
                added += 1;
            }
        }
        added
    }

    /// Stands everything visible `z_lift_mm` clear of the plate.
    ///
    /// Supports need room under a part, and the layers printed straight onto the plate
    /// are the ones that stick to it hardest. Returns how many objects moved.
    pub fn lift(&self, scene: &mut Scene) -> usize {
        let lift_mm = self.profile.z_lift_mm;
        let mut moved = 0;
        for object in scene.targets_mut() {
            let Some(bounds) = object.world_bounds() else {
                continue;
            };
            let offset = lift_over_plate(&bounds, lift_mm);
            if offset.z.abs() < f32::EPSILON {
                continue;
            }
            object.transform.translation += offset;
            moved += 1;
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Bvh, Mesh, Transform};
    use core_supports::SupportTree;
    use std::sync::Arc;

    /// A scene holding one unit cube standing on the plate.
    fn one_cube() -> Scene {
        use crate::scene::{ImportSummary, Imported};
        use core_geometry::{Orientation, diagnose};

        let cube = crate::supports::tests::cube();
        let summary = ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&cube),
        };

        let mut scene = Scene::default();
        scene.insert(Imported::new(
            "cube".to_owned(),
            Arc::new(cube),
            Transform::default(),
            summary,
        ));
        scene
    }

    /// An axis-aligned cube spanning 0..1, twelve triangles.
    fn cube() -> Mesh {
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

    /// The index of the first face of `mesh` whose normal points the given way.
    fn face_looking(mesh: &Mesh, up: Scalar) -> usize {
        (0..mesh.faces.len())
            .find(|face| {
                mesh.triangle(*face).is_some_and(|triangle| {
                    (triangle.normal_unnormalized().normalize_or_zero().z - up).abs() < 0.1
                })
            })
            .expect("a cube has a face looking each way")
    }

    /// Paints the surface the face `seed` belongs to on the only object of `scene`.
    fn paint_surface(scene: &mut Scene, seed: usize, blocking: bool) {
        let mesh = Arc::clone(&scene.objects()[0].mesh);
        let adjacency = Adjacency::of(&mesh);
        scene.objects_mut()[0]
            .supports
            .paint_face(&mesh, &adjacency, seed, 30.0, blocking, true);
    }

    #[test]
    fn filling_a_painted_patch_stands_supports_under_it() {
        let mut scene = one_cube();
        let underside = face_looking(&scene.objects()[0].mesh, -1.0);
        paint_surface(&mut scene, underside, false);

        let tool = SupportTool {
            fill: ProjectSettings {
                infill_spacing_mm: Some(0.3),
                border_spacing_mm: None,
            },
            ..SupportTool::default()
        };
        let added = tool.fill(&mut scene);

        assert!(
            added > 1,
            "a 1 mm square at 0.3 mm spacing takes several supports, got {added}"
        );
        for point in scene.objects()[0].supports.points() {
            assert!(
                point.contact.z.abs() < 1e-4,
                "{} is not on the underside that was painted",
                point.contact
            );
        }
    }

    #[test]
    fn a_painted_patch_is_drawable_before_the_columns_are_rebuilt() {
        let mut scene = one_cube();
        let underside = face_looking(&scene.objects()[0].mesh, -1.0);
        paint_surface(&mut scene, underside, false);

        let mesh = Arc::clone(&scene.objects()[0].mesh);
        scene.objects_mut()[0]
            .supports
            .refresh_patches(&mesh, Transform::default());

        let (painted, blocked) = scene.objects()[0].supports.patches();
        assert!(painted.is_some(), "the stroke has geometry to draw at once");
        assert!(blocked.is_none(), "nothing was painted out of bounds");
    }

    #[test]
    fn nothing_is_filled_where_the_patch_is_blocked() {
        let mut scene = one_cube();
        let underside = face_looking(&scene.objects()[0].mesh, -1.0);
        paint_surface(&mut scene, underside, false);
        paint_surface(&mut scene, underside, true);

        let tool = SupportTool::default();
        assert_eq!(
            tool.fill(&mut scene),
            0,
            "the painted patch is the blocked patch"
        );
    }

    #[test]
    fn a_support_over_a_blocked_face_steps_off_it() {
        let mut scene = one_cube();
        let lid = face_looking(&scene.objects()[0].mesh, 1.0);
        let mesh = Arc::clone(&scene.objects()[0].mesh);
        let bvh = Bvh::build(&mesh);
        scene.objects_mut()[0]
            .supports
            .add(Vec3::new(0.5, 0.5, 5.0), Transform::default(), 0);

        let rebuild = |scene: &mut Scene| {
            scene.objects_mut()[0].supports.refresh(
                &mesh,
                &bvh,
                Transform::default(),
                std::slice::from_ref(&SupportProfile::medium()),
            );
            scene.objects()[0].supports.trees()[0].landing().on_model
        };

        assert!(
            rebuild(&mut scene),
            "it stands on the lid of the cube while nothing is blocked"
        );

        paint_surface(&mut scene, lid, true);
        assert!(
            !rebuild(&mut scene),
            "the lid is painted out of bounds, so the support has to reach past it"
        );
    }

    #[test]
    fn only_the_two_brushes_paint() {
        assert!(Placing::Paint.paints());
        assert!(Placing::Block.paints());
        assert!(!Placing::Point.paints());
    }

    #[test]
    fn a_new_group_starts_from_the_numbers_in_hand_and_takes_the_pill() {
        let mut tool = SupportTool::default();
        tool.profile.middle.diameter_mm = 1.5;
        tool.add_group();

        assert_eq!(tool.active, 1);
        assert_eq!(tool.groups.len(), 2);
        assert!(
            (tool.table()[1].middle.diameter_mm - 1.5).abs() < 1e-6,
            "the new group was struck from what was being edited"
        );
    }

    #[test]
    fn choosing_another_group_keeps_what_was_edited_of_the_one_being_left() {
        let mut tool = SupportTool::default();
        tool.add_group();
        tool.profile.middle.diameter_mm = 2.5;

        tool.choose_group(0);
        assert!(
            (tool.profile.middle.diameter_mm - SupportProfile::default().middle.diameter_mm).abs()
                < 1e-6,
            "the default group's own numbers came back"
        );

        tool.choose_group(1);
        assert!(
            (tool.profile.middle.diameter_mm - 2.5).abs() < 1e-6,
            "the edit made to the second group was kept"
        );
    }

    #[test]
    fn dropping_a_group_takes_its_supports_away_with_it() {
        let mut scene = one_cube();
        let mut tool = SupportTool::default();
        tool.add_group();
        scene.objects_mut()[0]
            .supports
            .add(Vec3::new(0.5, 0.5, 5.0), Transform::default(), 1);
        assert_eq!(
            tool.group_holds(1, &scene),
            1,
            "the group holds one support"
        );

        assert_eq!(tool.remove_group(1, &mut scene), 1);

        assert_eq!(tool.groups.len(), 1);
        assert_eq!(tool.active, 0);
        assert!(
            scene.objects()[0].supports.points().is_empty(),
            "a support of a group that is gone is gone with it, not handed to another \
             group to be rebuilt in its shape"
        );
    }

    #[test]
    fn a_support_of_another_group_survives_the_one_that_is_dropped() {
        let mut scene = one_cube();
        let mut tool = SupportTool::default();
        tool.add_group();
        let object = &mut scene.objects_mut()[0];
        object
            .supports
            .add(Vec3::new(0.5, 0.5, 5.0), Transform::default(), 0);
        object
            .supports
            .add(Vec3::new(0.6, 0.6, 5.0), Transform::default(), 1);

        assert_eq!(tool.remove_group(1, &mut scene), 1);
        assert_eq!(
            scene.objects()[0].supports.point_count(),
            1,
            "the default group's own support stayed"
        );
    }

    /// Stands one support under the cube of `scene` and builds what it grows into.
    fn one_support(scene: &mut Scene, tool: &SupportTool) {
        scene.objects_mut()[0]
            .supports
            .add(Vec3::new(0.5, 0.5, 5.0), Transform::default(), 0);
        rebuild(scene, tool);
    }

    fn rebuild(scene: &mut Scene, tool: &SupportTool) {
        let mesh = Arc::clone(&scene.objects()[0].mesh);
        let bvh = Bvh::build(&mesh);
        scene.objects_mut()[0]
            .supports
            .refresh(&mesh, &bvh, Transform::default(), &tool.table());
    }

    #[test]
    fn freezing_a_support_takes_its_point_out_of_the_automatic_run() {
        let mut scene = one_cube();
        let tool = SupportTool::default();
        one_support(&mut scene, &tool);

        let frozen = scene.objects_mut()[0]
            .supports
            .freeze(0, Transform::default())
            .expect("the support was standing");
        rebuild(&mut scene, &tool);

        let supports = &scene.objects()[0].supports;
        assert_eq!(frozen, 0);
        assert!(supports.points().is_empty(), "the point went with the tree");
        assert_eq!(supports.frozen().len(), 1);
        assert_eq!(supports.trees().len(), 1, "it is still standing");
        assert_eq!(supports.point_count(), 1, "and still counted");
    }

    #[test]
    fn a_node_carried_aside_stays_there_through_a_rebuild() {
        let mut scene = one_cube();
        let tool = SupportTool::default();
        one_support(&mut scene, &tool);
        scene.objects_mut()[0]
            .supports
            .freeze(0, Transform::default())
            .expect("the support was standing");

        let moved = Vec3::new(2.0, 0.5, 5.0);
        scene.objects_mut()[0]
            .supports
            .frozen_mut(0)
            .expect("just frozen")
            .move_node(0, moved);
        rebuild(&mut scene, &tool);

        let standing = scene.objects()[0].supports.trees()[0].nodes()[0].position;
        assert!(
            standing.abs_diff_eq(moved, 1e-4),
            "the node stayed where it was carried, got {standing}"
        );
    }

    #[test]
    fn a_thawed_support_is_grown_again_from_its_tips() {
        let mut scene = one_cube();
        let tool = SupportTool::default();
        one_support(&mut scene, &tool);
        scene.objects_mut()[0]
            .supports
            .freeze(0, Transform::default())
            .expect("the support was standing");

        scene.objects_mut()[0]
            .supports
            .thaw(0, Transform::default());
        rebuild(&mut scene, &tool);

        let supports = &scene.objects()[0].supports;
        assert!(supports.frozen().is_empty());
        assert_eq!(supports.points().len(), 1, "its tip became a point again");
        assert_eq!(supports.trees().len(), 1);
    }

    #[test]
    fn the_default_group_cannot_be_dropped() {
        let mut scene = one_cube();
        let mut tool = SupportTool::default();
        tool.remove_group(0, &mut scene);
        assert_eq!(tool.groups.len(), 1, "something has to hold the raft");
    }

    #[test]
    fn two_groups_stand_two_thicknesses_on_one_model() {
        let mut scene = one_cube();
        let mut tool = SupportTool::default();
        tool.add_group();
        tool.profile.middle.diameter_mm *= 2.0;
        tool.groups[1].profile = tool.profile.clone();

        let object = &mut scene.objects_mut()[0];
        object
            .supports
            .add(Vec3::new(0.2, 0.5, 5.0), Transform::default(), 0);
        object
            .supports
            .add(Vec3::new(0.8, 0.5, 5.0), Transform::default(), 1);

        let mesh = Arc::clone(&scene.objects()[0].mesh);
        let bvh = Bvh::build(&mesh);
        scene.objects_mut()[0]
            .supports
            .refresh(&mesh, &bvh, Transform::default(), &tool.table());

        let groups: Vec<u16> = scene.objects()[0]
            .supports
            .trees()
            .iter()
            .map(SupportTree::group)
            .collect();
        assert_eq!(
            groups,
            vec![0, 1],
            "one tree per group, each to its own shape"
        );
    }

    #[test]
    fn lifting_stands_a_model_its_lift_clear_of_the_plate() {
        let mut scene = one_cube();
        let tool = SupportTool::default();

        assert_eq!(tool.lift(&mut scene), 1);
        let bounds = scene.objects()[0]
            .world_bounds()
            .expect("the cube has bounds");
        assert!(
            (bounds.mins.z - tool.profile.z_lift_mm).abs() < 1e-4,
            "the lowest point ends up at the lift, got {}",
            bounds.mins.z
        );
    }

    #[test]
    fn lifting_a_model_that_is_already_there_moves_nothing() {
        let mut scene = one_cube();
        let tool = SupportTool::default();
        tool.lift(&mut scene);
        assert_eq!(
            tool.lift(&mut scene),
            0,
            "a second lift must not stack on the first"
        );
    }

    #[test]
    fn a_hidden_model_is_not_lifted() {
        let mut scene = one_cube();
        scene.objects_mut()[0].visible = false;
        assert_eq!(SupportTool::default().lift(&mut scene), 0);
    }
}

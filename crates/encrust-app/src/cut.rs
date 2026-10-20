use anyhow::{Result, anyhow};
use core_geometry::{Mesh, Plane, Scalar, Transform, Vec3, transform_mesh};
use core_joint::{Dowel, JointKind, Joints};
use core_volume::PartedOff;

pub use core_engine::project::Keep;

use core_engine::project::{Axis, CutState};

use crate::i18n::tr;
use crate::job::{
    Counted, CutJob, CutOutcome, CutRequest, SplitJob, SplitOutcome, SplitRequest, spawn_cut,
    spawn_split,
};
use crate::landing::{self, mending, place, place_all};
use crate::scene::{ObjectId, Scene, SceneObject, Taken};
use crate::status::Status;

/// Space left between a cut's lower half and the dowels laid out beside it, and between
/// one dowel and the next, millimetres.
const DOWEL_GAP_MM: Scalar = 3.0;

/// Where the Cut tool's plane is, and what it does with the halves.
#[derive(Debug)]
pub struct CutTool {
    /// What the panel is set to, in the shape a project writes it down in (ADR 0191),
    /// joints included (ADR 0306). Its `height_mm` is where the plane crosses the axis:
    /// across Z the height above the plate, across X or Y the distance from the model's
    /// centre of mass.
    pub state: CutState,
    /// Whether each half is turned to stand on the face the cut left. The window's own,
    /// like a view switch: a project does not write it down.
    pub lay_flat: bool,
    /// The model the plane was last put through the middle of, so picking another one
    /// moves the plane to it.
    pub centred_on: Option<ObjectId>,
    running: Option<Running>,
    /// A split under way, and the model it splits.
    splitting: Option<(SplitJob, Taken)>,
    /// Whether the next click on a model marks the part Split cuts off it.
    pub picking_part: bool,
    /// The model a part is marked on, and the point, in the model's own coordinates so
    /// that it moves with it. A mark on a mesh that has since been replaced marks nothing.
    part_mark: Option<(Taken, Vec3)>,
}

/// A cut under way, with what the tool was set to when it started.
#[derive(Debug)]
struct Running {
    job: CutJob,
    taken: Taken,
    axis: Axis,
    position_mm: Scalar,
    up: Vec3,
    keep: Keep,
    joints: Joints,
    lay_flat: bool,
}

impl Default for CutTool {
    fn default() -> Self {
        Self {
            state: CutState {
                axis: Axis::Z,
                height_mm: 10.0,
                keep: Keep::Both,
                joints: Joints::default(),
                parts: core_volume::PartSettings::default(),
            },
            lay_flat: false,
            centred_on: None,
            running: None,
            splitting: None,
            picking_part: false,
            part_mark: None,
        }
    }
}

impl CutTool {
    /// The plane the tool is set to for one model, in plate coordinates.
    pub fn plane(&self, object: &SceneObject) -> Option<Plane> {
        Plane::at(normal(self.state.axis), self.position_mm(object))
    }

    /// Where the plane crosses its axis for one model, in plate millimetres.
    pub fn position_mm(&self, object: &SceneObject) -> Scalar {
        match self.state.axis {
            Axis::X => object.pivot().translation.x + self.state.height_mm,
            Axis::Y => object.pivot().translation.y + self.state.height_mm,
            Axis::Z => self.state.height_mm,
        }
    }

    /// Turns the plane to `axis`, through the model's centre of mass across X or Y and
    /// halfway up it across Z: the offset meant for one axis means nothing on another.
    pub fn set_axis(&mut self, axis: Axis, object: &SceneObject) {
        self.state.axis = axis;
        self.state.height_mm = match axis {
            Axis::X | Axis::Y => 0.0,
            Axis::Z => object
                .world_bounds()
                .map_or(object.pivot().translation.z, |bounds| {
                    (bounds.mins.z + bounds.maxs.z) / 2.0
                }),
        };
    }

    /// Puts the plane through the middle of `id` on the axis it is set to, unless it was
    /// already put through this model.
    pub fn centre_on(&mut self, id: ObjectId, object: &SceneObject) {
        if self.centred_on != Some(id) {
            self.centred_on = Some(id);
            self.set_axis(self.state.axis, object);
        }
    }

    /// Starts cutting one model in two off the drawing thread; [`CutTool::poll`] puts the
    /// halves, new geometry with none of its supports, cavity or holes, in its place.
    pub fn start(&mut self, scene: &Scene, id: ObjectId) -> Result<()> {
        let object = scene
            .get(id)
            .ok_or_else(|| anyhow!(tr!("cut-nothing-selected")))?;
        let plane = self
            .plane(object)
            .ok_or_else(|| anyhow!(tr!("cut-no-direction")))?;
        let shell = object.hollow.shell();
        let printed = transform_mesh(shell.unwrap_or(&object.mesh), object.transform);
        let solid = shell.map(|_| transform_mesh(&object.mesh, object.transform));
        let job = spawn_cut(CutRequest {
            printed,
            solid,
            plane,
            keep: self.state.keep,
            joints: self.state.joints,
        });
        self.running = Some(Running {
            job,
            taken: Taken::of(object),
            axis: self.state.axis,
            position_mm: self.position_mm(object),
            up: plane.normal,
            keep: self.state.keep,
            joints: self.state.joints,
            lay_flat: self.lay_flat,
        });
        Ok(())
    }

    /// Lands a finished cut in the scene. Returns whether one is still going, which is
    /// what tells the window to keep repainting.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        if let Some(outcome) = self.running.as_mut().and_then(|running| running.job.poll())
            && let Some(running) = self.running.take()
        {
            *status = running.land(scene, outcome);
        }
        if let Some(outcome) = self.splitting.as_mut().and_then(|(job, _)| job.poll())
            && let Some((_, taken)) = self.splitting.take()
        {
            if matches!(outcome, SplitOutcome::PartedOff(_)) {
                self.part_mark = None;
            }
            *status = land_parts(scene, &taken, outcome);
        }
        self.is_busy()
    }

    pub fn is_busy(&self) -> bool {
        self.running.is_some() || self.splitting.is_some()
    }

    /// How far the split going has got, while it has counted something; a cut counts
    /// nothing.
    pub fn counted(&self) -> Option<Counted> {
        self.splitting.as_ref()?.0.counted()
    }

    /// Gives up on the cut or split going, which leaves the model as it is.
    pub fn cancel(&mut self, status: &mut Status) {
        if self.running.take().is_some() | self.splitting.take().is_some() {
            *status = Status::Info(tr!("cut-cancelled"));
        }
    }

    /// Marks the part at `point` on the plate as the one Split cuts off `object`, and
    /// ends the picking.
    pub fn mark_part(&mut self, object: &SceneObject, point: Vec3) {
        let local = object
            .transform
            .to_matrix()
            .inverse()
            .transform_point3(point);
        self.part_mark = Some((Taken::of(object), local));
        self.picking_part = false;
    }

    /// Where the part is marked on `object`, in plate coordinates, if it is.
    pub fn marked_on(&self, object: &SceneObject) -> Option<Vec3> {
        let (marked, local) = self.part_mark.as_ref()?;
        marked
            .is_model(object)
            .then(|| object.transform.to_matrix().transform_point3(*local))
    }

    /// Forgets the part marked, as the toggle that marked it does.
    pub fn unmark_part(&mut self) {
        self.part_mark = None;
    }

    /// Starts breaking one model into the parts it is made of, or cutting off the part
    /// marked on it, off the drawing thread, as the tool's `parts` settings say;
    /// [`CutTool::poll`] puts them on the plate.
    pub fn start_split(&mut self, scene: &Scene, id: ObjectId) -> Result<()> {
        let object = scene
            .get(id)
            .ok_or_else(|| anyhow!(tr!("cut-nothing-to-split")))?;
        let mesh = object.hollow.shell().unwrap_or(&object.mesh);
        let job = spawn_split(SplitRequest {
            placed: transform_mesh(mesh, object.transform),
            settings: self.state.parts,
            at: self.marked_on(object),
        });
        self.splitting = Some((job, Taken::of(object)));
        Ok(())
    }
}

impl Running {
    fn land(self, scene: &mut Scene, outcome: CutOutcome) -> Status {
        let name = self.taken.name.clone();
        let (joined, open_loops, unjoined) = match outcome {
            CutOutcome::Cut {
                joined,
                open_loops,
                unjoined,
            } => (joined, open_loops, unjoined),
            CutOutcome::Missed => return Status::Error(tr!("cut-misses", name = name)),
            CutOutcome::Failed(message) => return Status::Error(message),
        };
        if let Err(changed) = landing::take_off(scene, std::slice::from_ref(&self.taken)) {
            return Status::Error(changed);
        }
        let kind = self.joints.kind.filter(|_| self.keep == Keep::Both);
        // Pins and a tongue stand on the lower half's cut, which it then cannot lie on.
        let raised = matches!(kind, Some(JointKind::Pins | JointKind::Key)) && joined.placed > 0;
        let (mended, lower) = self.put_down(scene, joined.below, joined.above, raised);
        if let Some(lower) = lower {
            lay_out_dowels(scene, lower, &name, &joined.loose);
        }
        if open_loops > 0 {
            let said = tr!("cut-done-open", name = name, open = open_loops);
            return Status::noted(said, mending(mended).into_iter().collect());
        }
        let mut notes = joint_notes(kind, joined.placed, joined.bare, unjoined);
        notes.extend(mending(mended));
        let said = tr!(
            "cut-done",
            name = name,
            axis = self.axis.label(),
            position = format!("{:.1}", self.position_mm)
        );
        Status::noted(said, notes)
    }

    /// Puts the halves kept on the plate, each laid on its cut face if asked and unless
    /// it is the lower one and `raised`. Answers how many had to be mended, and the lower.
    fn put_down(
        &self,
        scene: &mut Scene,
        below: Mesh,
        above: Mesh,
        raised: bool,
    ) -> (usize, Option<ObjectId>) {
        let name = &self.taken.name;
        let up = self.up;
        let pieces = match self.keep {
            Keep::Both => vec![
                (tr!("cut-lower-name", name = name.clone()), below, up),
                (tr!("cut-upper-name", name = name.clone()), above, -up),
            ],
            Keep::Below => vec![(name.clone(), below, up)],
            Keep::Above => vec![(name.clone(), above, -up)],
        };
        let (mut mended, mut lower) = (0, None);
        for (name, mesh, cut_face) in pieces {
            let (piece, was_mended) = place(scene, self.taken.plate, name, mesh);
            mended += usize::from(was_mended);
            let is_lower = cut_face == up;
            if is_lower {
                lower = Some(piece);
            }
            if let Some(object) = scene.get_mut(piece)
                && self.lay_flat
                && !(is_lower && raised)
            {
                object.lay_face_down(cut_face);
            }
        }
        (mended, lower)
    }
}

/// Stands each dowel on the plate in a row beside the lower half `beside`.
fn lay_out_dowels(scene: &mut Scene, beside: ObjectId, name: &str, dowels: &[Dowel]) {
    let Some((bounds, plate)) = scene
        .get(beside)
        .and_then(|object| Some((object.world_bounds()?, object.plate)))
    else {
        return;
    };
    let mut x = bounds.maxs.x + DOWEL_GAP_MM;
    for (index, dowel) in dowels.iter().enumerate() {
        let radius = dowel.radius_mm;
        let offset = Vec3::new(x + radius, bounds.mins.y + radius, 0.0);
        let placed = transform_mesh(&dowel.mesh, Transform::from_translation(offset));
        let name = tr!("cut-dowel-name", name = name.to_owned(), index = index + 1);
        place(scene, plate, name, placed);
        x += 2.0 * radius + DOWEL_GAP_MM;
    }
}

/// What the stage notice notes of the joint, and of the pieces of the face it missed.
fn joint_notes(
    kind: Option<JointKind>,
    placed: usize,
    bare: usize,
    unjoined: Option<String>,
) -> Vec<String> {
    let Some(kind) = kind else {
        return Vec::new();
    };
    if let Some(reason) = unjoined {
        return vec![tr!("cut-joint-failed", reason = reason)];
    }
    let joint = match (kind, placed) {
        (_, 0) => return vec![tr!("cut-no-room-for-joint")],
        (JointKind::Pins, count) => tr!("cut-with-pins", count = count),
        (JointKind::Dowels, count) => tr!("cut-with-dowels", count = count),
        (JointKind::Magnets, count) => tr!("cut-with-magnets", count = count),
        (JointKind::Key, count) => tr!("cut-with-key", count = count),
    };
    let bare = (bare > 0).then(|| tr!("cut-bare", count = bare));
    std::iter::once(joint).chain(bare).collect()
}

/// Puts the parts a split found on the plate in place of the model it took. A model that
/// is one part and lost no scrap is left alone, supports and all.
fn land_parts(scene: &mut Scene, taken: &Taken, outcome: SplitOutcome) -> Status {
    let name = &taken.name;
    let parts = match outcome {
        SplitOutcome::Split(parts) => parts,
        SplitOutcome::PartedOff(parted) => return land_parted_off(scene, taken, parted),
        SplitOutcome::Failed(message) => return Status::Error(message),
    };
    let count = parts.parts.len();
    if count < 2 && parts.joined + parts.dropped == 0 {
        return Status::Info(tr!("cut-one-piece", name = name.clone()));
    }
    if let Err(changed) = landing::take_off(scene, std::slice::from_ref(taken)) {
        return Status::Error(changed);
    }
    let mut notes = scrap_notes(parts.joined, parts.dropped);
    let named = parts.parts.into_iter().enumerate().map(|(index, part)| {
        let part_name = match count {
            1 => name.clone(),
            _ => tr!("cut-part-name", name = name.clone(), index = index + 1),
        };
        (part_name, part)
    });
    notes.extend(mending(place_all(scene, taken.plate, named)));
    let said = match count {
        0 | 1 => tr!("cut-cleaned", name = name.clone()),
        count => tr!("cut-split-done", name = name.clone(), count = count),
    };
    Status::noted(said, notes)
}

/// Puts the marked part and the rest of the model on the plate in its place.
fn land_parted_off(scene: &mut Scene, taken: &Taken, parted: PartedOff) -> Status {
    if let Err(changed) = landing::take_off(scene, std::slice::from_ref(taken)) {
        return Status::Error(changed);
    }
    let name = &taken.name;
    let sides = [parted.part, parted.rest]
        .into_iter()
        .enumerate()
        .map(|(index, side)| {
            let side_name = tr!("cut-part-name", name = name.clone(), index = index + 1);
            (side_name, side)
        });
    let mended = place_all(scene, taken.plate, sides);
    let said = tr!("cut-parted-off", name = name.clone());
    Status::noted(said, mending(mended).into_iter().collect())
}

/// What the stage notice notes of the scraps a split joined to their parts or dropped.
fn scrap_notes(joined: usize, dropped: usize) -> Vec<String> {
    let joined = (joined > 0).then(|| tr!("cut-split-joined", count = joined));
    let dropped = (dropped > 0).then(|| tr!("cut-split-dropped", count = dropped));
    joined.into_iter().chain(dropped).collect()
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
    use crate::scene::{ImportSummary, Imported};
    use core_geometry::{diagnose, signed_volume};
    use std::sync::Arc;

    /// The tool set to one plane: where it crosses `axis`, and which halves to keep.
    fn at(axis: Axis, height_mm: Scalar, keep: Keep) -> CutTool {
        CutTool {
            state: CutState {
                axis,
                height_mm,
                keep,
                joints: Joints::default(),
                parts: core_volume::PartSettings::default(),
            },
            ..CutTool::default()
        }
    }

    /// Runs the cut `tool` is set to through to the end, as the window's frames would.
    fn cut_now(mut tool: CutTool, scene: &mut Scene, id: ObjectId) -> Status {
        if let Err(error) = tool.start(scene, id) {
            return Status::Error(error.to_string());
        }
        let mut status = Status::default();
        while tool.poll(scene, &mut status) {
            std::thread::yield_now();
        }
        status
    }

    /// Runs a split of `id` through to the end, as the window's frames would.
    fn split_now(scene: &mut Scene, id: ObjectId) -> Status {
        let mut tool = CutTool::default();
        if let Err(error) = tool.start_split(scene, id) {
            return Status::Error(error.to_string());
        }
        let mut status = Status::default();
        while tool.poll(scene, &mut status) {
            std::thread::yield_now();
        }
        status
    }

    /// Four 4 mm pegs 5 mm long of `kind`, two to a piece, all of a width.
    fn joined_by(kind: JointKind) -> Joints {
        Joints {
            kind: Some(kind),
            pegs: core_joint::Pegs {
                auto_size: false,
                diameter_mm: 4.0,
                length_mm: 5.0,
                keyed: false,
                ..core_joint::Pegs::default()
            },
            ..Joints::default()
        }
    }

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
            ImportSummary::made(diagnose(&mesh)),
        ));
        (scene, id)
    }

    #[test]
    fn a_cut_leaves_two_models_where_there_was_one() {
        let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), Transform::default());
        let status = cut_now(at(Axis::Z, 4.0, Keep::Both), &mut scene, id);
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
        let status = cut_now(at(Axis::Z, 4.0, Keep::Both), &mut scene, id);
        assert!(
            status.is_error(),
            "a plane that misses the model cuts nothing"
        );
        assert_eq!(scene.objects().len(), 1, "and the model is left alone");
    }

    #[test]
    fn a_half_laid_flat_stands_on_its_cut_face() {
        let (mut scene, id) = scene_with(cuboid(Vec3::new(10.0, 10.0, 20.0)), Transform::default());
        let tool = CutTool {
            lay_flat: true,
            ..at(Axis::X, 0.0, Keep::Below)
        };
        cut_now(tool, &mut scene, id);

        // The half is 5 mm across X; lying on the face the cut left, that is its height.
        let bounds = scene.objects()[0]
            .world_bounds()
            .expect("a half has bounds");
        let height = bounds.maxs.z - bounds.mins.z;
        assert!((height - 5.0).abs() < 1e-3, "got {height}");
        assert!(bounds.mins.z.abs() < 1e-3, "and stands on the plate");
    }

    #[test]
    fn pins_stand_on_a_lower_half_that_stays_upright_and_sockets_go_into_the_upper() {
        let (mut scene, id) = scene_with(cuboid(Vec3::new(40.0, 10.0, 20.0)), Transform::default());
        let mut tool = at(Axis::Z, 10.0, Keep::Both);
        tool.state.joints = joined_by(JointKind::Pins);
        tool.lay_flat = true;
        let status = cut_now(tool, &mut scene, id);
        let Status::Noted(_, notes) = &status else {
            panic!("a block through the middle is a cut, got {status:?}");
        };
        assert_eq!(
            notes,
            &[tr!("cut-with-pins", count = 2)],
            "two pins are named"
        );

        let [lower, upper] = scene.objects() else {
            panic!("two halves, got {}", scene.objects().len());
        };
        let top = lower.world_bounds().expect("a half has bounds").maxs.z;
        assert!(
            (top - 15.0).abs() < 1e-3,
            "5 mm pins stand on the 10 mm cut, so it is not laid on it, got {top}"
        );
        assert!(
            lower.summary.diagnostics.is_sound(),
            "a pin is welded into its half"
        );
        assert!(
            signed_volume(&upper.mesh) < 4000.0 - 2.0 * 50.0,
            "two sockets of some 4.2 mm by 5.3 mm are taken out of the upper half"
        );
    }

    #[test]
    fn dowels_stand_on_the_plate_beside_the_lower_half() {
        let (mut scene, id) = scene_with(cuboid(Vec3::new(40.0, 10.0, 20.0)), Transform::default());
        let mut tool = at(Axis::Z, 10.0, Keep::Both);
        tool.state.joints = joined_by(JointKind::Dowels);
        let status = cut_now(tool, &mut scene, id);
        assert!(!status.is_error(), "got {status:?}");

        let [lower, _, dowels @ ..] = scene.objects() else {
            panic!("two halves and their dowels");
        };
        assert_eq!(dowels.len(), 2, "one dowel a pair of sockets");
        let edge = lower.world_bounds().expect("a half has bounds").maxs.x;
        for dowel in dowels {
            let bounds = dowel.world_bounds().expect("a dowel has bounds");
            assert!(
                bounds.mins.z.abs() < 1e-3,
                "{} stands on the plate",
                dowel.name
            );
            assert!(bounds.mins.x > edge, "{} is clear of the half", dowel.name);
        }
    }

    #[test]
    fn keeping_one_half_throws_the_other_away() {
        let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), Transform::default());
        cut_now(at(Axis::Z, 6.0, Keep::Below), &mut scene, id);
        assert_eq!(scene.objects().len(), 1);
        let volume = signed_volume(&scene.objects()[0].mesh);
        assert!((volume - 600.0).abs() < 1e-1, "got {volume}");
    }

    #[test]
    fn cutting_across_the_other_axes_is_the_same_cut() {
        // The cube's centre of mass is at 5 mm, so 2 mm short of it is 3 mm into the cube.
        for (axis, expected) in [(Axis::X, 300.0), (Axis::Y, 300.0)] {
            let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), Transform::default());
            cut_now(at(axis, -2.0, Keep::Below), &mut scene, id);

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
        let status = cut_now(at(Axis::X, 0.0, Keep::Below), &mut scene, id);
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
    fn turning_the_plane_puts_it_through_the_middle_of_the_model() {
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
    fn picking_a_model_puts_the_plane_halfway_up_it_once() {
        let (scene, id) = scene_with(cuboid(Vec3::new(10.0, 10.0, 30.0)), Transform::default());
        let object = scene.get(id).expect("the box was just inserted");
        let mut tool = CutTool::default();

        tool.centre_on(id, object);
        assert!(
            (tool.position_mm(object) - 15.0).abs() < 1e-4,
            "half of a 30 mm tall box"
        );
        tool.state.height_mm = 4.0;
        tool.centre_on(id, object);
        assert!(
            (tool.state.height_mm - 4.0).abs() < 1e-6,
            "a height typed for the same model stays"
        );
    }

    /// The cuboid with the two faces of its top missing: every half of it comes off the
    /// cut open, as a model does whose surface was already torn.
    fn open_cuboid(size: Vec3) -> Mesh {
        let mut mesh = cuboid(size);
        mesh.faces
            .retain(|face| !matches!(face, [4, 5, 6] | [4, 6, 7]));
        mesh
    }

    #[test]
    fn a_half_that_comes_off_the_cut_broken_is_mended_before_it_lands() {
        let open = open_cuboid(Vec3::splat(10.0));
        assert!(
            !diagnose(&open).is_sound(),
            "the model is open before it is cut"
        );
        let (mut scene, id) = scene_with(open, Transform::default());

        let status = cut_now(at(Axis::Z, 4.0, Keep::Both), &mut scene, id);

        assert!(!status.is_error());
        assert_eq!(scene.objects().len(), 2);
        for object in scene.objects() {
            assert!(
                object.summary.is_sound(),
                "{} came down mended, got {:?}",
                object.name,
                object.summary.defects()
            );
        }
    }

    #[test]
    fn a_half_that_comes_off_the_cut_sound_is_left_alone() {
        let (mut scene, id) = scene_with(cuboid(Vec3::splat(10.0)), Transform::default());

        let status = cut_now(at(Axis::Z, 4.0, Keep::Both), &mut scene, id);
        assert!(
            matches!(status, Status::Info(_)),
            "a cut with nothing mended notes nothing, got {status:?}"
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
        let status = split_now(&mut scene, id);
        assert!(!status.is_error(), "got {status:?}");
        assert_eq!(scene.objects().len(), 2);
    }

    /// Splits `id` with a part marked at `at` on the plate, through to the end.
    fn part_off_now(tool: &mut CutTool, scene: &mut Scene, id: ObjectId, at: Vec3) -> Status {
        let object = scene.get(id).expect("the model is on the plate");
        tool.mark_part(object, at);
        if let Err(error) = tool.start_split(scene, id) {
            return Status::Error(error.to_string());
        }
        let mut status = Status::default();
        while tool.poll(scene, &mut status) {
            std::thread::yield_now();
        }
        status
    }

    #[test]
    fn a_part_marked_on_a_moved_dumbbell_is_cut_off_and_the_mark_is_spent() {
        let solid = |min: Vec3, max: Vec3| {
            core_csg::Solid::new(&core_geometry::cuboid(min, max)).expect("a box is a solid")
        };
        let dumbbell = solid(Vec3::ZERO, Vec3::splat(10.0))
            .union(&solid(Vec3::new(10.0, 4.0, 4.0), Vec3::new(20.0, 6.0, 6.0)))
            .and_then(|joined| joined.union(&solid(Vec3::new(20.0, 0.0, 0.0), Vec3::splat(30.0))))
            .expect("three boxes in a row join")
            .mesh();
        let moved = Transform::from_translation(Vec3::new(5.0, 0.0, 0.0));
        let (mut scene, id) = scene_with(dumbbell, moved);
        let mut tool = CutTool::default();
        let status = part_off_now(&mut tool, &mut scene, id, Vec3::new(5.0, 5.0, 5.0));
        assert!(!status.is_error(), "got {status:?}");
        assert!(
            tool.part_mark.is_none(),
            "the mark went with the model it was on"
        );
        let [part, rest] = scene.objects() else {
            panic!("the marked part and the rest replace the model");
        };
        let ends = |object: &SceneObject| object.world_bounds().expect("a side has faces");
        assert!(
            ends(part).maxs.x < 25.0 && ends(rest).mins.x > 15.0,
            "the part is the box the mark is on, cut across the bar 15 to 25 mm on the plate"
        );
    }

    #[test]
    fn a_part_marked_on_a_box_has_no_neck_and_the_box_stays() {
        let (mut scene, id) = scene_with(cuboid(Vec3::splat(4.0)), Transform::default());
        let mut tool = CutTool::default();
        let status = part_off_now(&mut tool, &mut scene, id, Vec3::new(0.0, 2.0, 2.0));
        assert_eq!(status, Status::Error(tr!("cut-no-neck")));
        assert_eq!(scene.objects().len(), 1);
    }

    #[test]
    fn splitting_one_solid_leaves_it_alone() {
        let (mut scene, id) = scene_with(cuboid(Vec3::splat(4.0)), Transform::default());
        let status = split_now(&mut scene, id);
        assert!(!status.is_error());
        assert_eq!(scene.objects().len(), 1);
        assert_eq!(scene.objects()[0].id, id, "the model is the same one");
    }

    #[test]
    fn a_scrap_hanging_beside_a_model_is_dropped_and_the_model_kept_as_one() {
        let mut model = cuboid(Vec3::splat(4.0));
        let scrap = cuboid(Vec3::splat(0.5));
        let offset = model.vertices.len() as u32;
        model.vertices.extend(
            scrap
                .vertices
                .iter()
                .map(|v| *v + Vec3::new(10.0, 0.0, 0.0)),
        );
        model.faces.extend(
            scrap
                .faces
                .iter()
                .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
        );
        let (mut scene, id) = scene_with(model, Transform::default());
        let status = split_now(&mut scene, id);
        let Status::Noted(_, notes) = &status else {
            panic!("a split that dropped a scrap notes it, got {status:?}");
        };
        let [object] = scene.objects() else {
            panic!("the scrap is no part of its own");
        };
        assert_eq!(object.name, "cube", "the model keeps its name");
        assert!((signed_volume(&object.mesh) - 64.0).abs() < 1e-3);
        assert_eq!(notes, &[tr!("cut-split-dropped", count = 1)]);
    }

    #[test]
    fn a_part_marked_on_a_mesh_since_replaced_marks_nothing() {
        let (mut scene, id) = scene_with(cuboid(Vec3::splat(4.0)), Transform::default());
        let mut tool = CutTool::default();
        let object = scene.get(id).expect("the model is on the plate");
        tool.mark_part(object, Vec3::new(0.0, 2.0, 2.0));
        assert!(tool.marked_on(object).is_some());
        let replaced = crate::scene::Shaped::of(Arc::new(cuboid(Vec3::splat(6.0))));
        let object = scene.get_mut(id).expect("the model is on the plate");
        object.reshape(replaced);
        assert_eq!(
            tool.marked_on(object),
            None,
            "the mark was on the mesh that went"
        );
    }
}

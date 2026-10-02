use std::sync::Arc;

use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3};

use crate::{
    Blocker, Channel, DrainHole, HollowSettings, bores, channel_under, drill, hole_at, lift_for,
    pierce, sleeves,
};

/// A transform this close to singular has flattened its model, and a click on it cannot
/// be mapped back into the model's own space.
const SINGULAR: f32 = 1e-12;

/// How wide a point of a channel still being laid out is drawn, in millimetres.
const PENDING_MARKER_MM: Scalar = 0.6;

/// Rings and segments of the ball a marker is drawn as. A marker is read as a position,
/// not as a shape, so it is meshed as coarsely as one can be and still look round.
const MARKER_RINGS: usize = 8;
const MARKER_SEGMENTS: usize = 12;

/// The cavity one model carries: where its wall is to stay solid, the holes and channels
/// cut into it, and the shell built from it. All of it is in the model's own space, so it
/// travels with the model; see `docs/decisions/0129`.
#[derive(Debug, Clone, Default)]
pub struct ModelHollow {
    /// In the model's own space, so that they travel with it; see
    /// `docs/decisions/0028-supports-are-vertical-columns-anchored-in-model-space.md`,
    /// which anchors supports the same way.
    blockers: Vec<Blocker>,
    /// Also in the model's own space, so that a hole travels with what it was drilled in.
    drains: Vec<DrainHole>,
    /// Channels dug through the model, and the points of the one being laid out.
    channels: Vec<Channel>,
    /// Each point of the channel being laid out with the outward normal it was clicked on.
    pending: Vec<(Vec3, Vec3)>,
    /// The blockers and the points of the channel being laid out as balls, meshed for a
    /// viewport. Kept so that a renderer's cache is not churned when nothing moved.
    markers: Option<Arc<Mesh>>,
    /// The holes and channels as they are actually cut, and the bodies they cut with:
    /// deepened through the wall once the model is hollow, and in the model's own space.
    /// A cut is its own operation and does not wait for a cavity; see ADR 0075.
    deepened: Vec<DrainHole>,
    cuts: Option<Arc<Mesh>>,
    /// The same cuts seen from inside, for the viewport alone: a hole with no wall drawn
    /// in it is a window rather than a hole. See ADR 0073.
    bores: Option<Arc<Mesh>>,
    /// The model those walls are clipped against, shared with the object that owns it. A
    /// cut is placed on the model as it was imported, and so is what is drawn inside it.
    model: Option<(Arc<Mesh>, Arc<Bvh>)>,
    built: Option<Shell>,
}

/// A model hollowed, in its own space: what a run of `hollow` hands back to be kept.
#[derive(Debug, Clone, PartialEq)]
pub struct Shell {
    /// The outer surface, the cavity wound inward and its infill.
    pub mesh: Arc<Mesh>,
    pub cavity_mm3: Scalar,
    /// The lattice the cavity came out on, millimetres, and whether the memory budget
    /// made it coarser than the precision asked for.
    pub voxel_mm: Scalar,
    pub coarsened: bool,
    /// The scale the model stood at, which the wall was measured under.
    pub scale: Vec3,
    pub settings: HollowSettings,
}

/// The hole a click drills: its mouth and depth in millimetres of the plate, and its far
/// end as a fraction of its mouth, 1 being a cylinder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoleSize {
    pub diameter_mm: Scalar,
    pub depth_mm: Scalar,
    pub taper: Scalar,
}

impl ModelHollow {
    /// The model every cut on it is measured against. Given once, when the object is
    /// imported, because a wall drawn inside a hole has to be clipped to it.
    pub fn on(mesh: Arc<Mesh>, bvh: Arc<Bvh>) -> Self {
        Self {
            model: Some((mesh, bvh)),
            ..Self::default()
        }
    }

    /// The cuts a project file was saved with, on the model they were placed on. The
    /// cavity itself is not in the file: it is asked for again by the Hollow tool.
    pub fn restored(
        mesh: Arc<Mesh>,
        bvh: Arc<Bvh>,
        blockers: Vec<Blocker>,
        drains: Vec<DrainHole>,
        channels: Vec<Channel>,
    ) -> Self {
        let mut hollow = Self {
            blockers,
            drains,
            channels,
            ..Self::on(mesh, bvh)
        };
        hollow.recut();
        hollow
    }

    pub fn blockers(&self) -> &[Blocker] {
        &self.blockers
    }

    pub fn drains(&self) -> &[DrainHole] {
        &self.drains
    }

    /// The holes and channels as they are cut: the holes reach further than the depth
    /// they were asked at wherever they have a wall to go through.
    pub fn cut(&self) -> (&[DrainHole], &[Channel]) {
        (&self.deepened, &self.channels)
    }

    /// The bodies those cuts take out of this model, wound inward and in the model's own
    /// space, or `None` when nothing has been placed.
    pub fn cut_bodies(&self) -> Option<&Arc<Mesh>> {
        self.cuts.as_ref()
    }

    /// The walls and floors of those cuts, in the same space: what the viewport draws
    /// inside a hole, and what nothing else ever sees.
    pub fn bore(&self) -> Option<&Arc<Mesh>> {
        self.bores.as_ref()
    }

    /// Drills a hole from the nearest surface into `target`, a point in the model's own
    /// space, deep enough to reach it: what gives a trapped pocket its way out.
    ///
    /// `diameter_mm` is measured on the plate, like the one a click drops. Returns whether
    /// there was a surface to drill through at all.
    pub fn drill_into(
        &mut self,
        mesh: &Mesh,
        bvh: &Bvh,
        target: Vec3,
        diameter_mm: Scalar,
        taper: Scalar,
        transform: Transform,
    ) -> bool {
        let scale = transform.scale.abs();
        let smallest = scale.x.min(scale.y).min(scale.z).max(f32::EPSILON);
        let diameter_mm = diameter_mm / smallest;
        let Some(hole) = hole_at(mesh, bvh, target, diameter_mm, 0.0, taper) else {
            return false;
        };

        // A diameter past the pocket's floor, so the tip is inside the pocket rather than
        // on its ceiling.
        self.drains.push(DrainHole {
            depth_mm: (target - hole.at).length() + diameter_mm,
            ..hole
        });
        self.recut();
        true
    }

    /// The blockers as balls in the model's own space, for the viewport to draw, or
    /// `None` when there are none.
    pub fn markers(&self) -> Option<&Arc<Mesh>> {
        self.markers.as_ref()
    }

    pub fn is_hollow(&self) -> bool {
        self.built.is_some()
    }

    /// The whole hollowed model in its own space — the outer surface, the cavity and its
    /// infill — or `None` while the model is still solid.
    pub fn shell(&self) -> Option<&Arc<Mesh>> {
        self.built.as_ref().map(|built| &built.mesh)
    }

    /// Resin the cavity takes out of this model, in cubic millimetres.
    pub fn cavity_mm3(&self) -> Scalar {
        self.built.as_ref().map_or(0.0, |built| built.cavity_mm3)
    }

    /// The lattice the cavity was cut on, and whether the memory budget coarsened it.
    pub fn lattice(&self) -> Option<(Scalar, bool)> {
        self.built
            .as_ref()
            .map(|built| (built.voxel_mm, built.coarsened))
    }

    /// Whether what is built no longer matches what the tool is asking for, or the model
    /// has been rescaled under it. A wall is measured in the model's own space, so a
    /// model scaled after it was hollowed has a wall of the wrong thickness.
    pub fn is_stale(&self, asked: &HollowSettings, transform: Transform) -> bool {
        self.built.as_ref().is_some_and(|built| {
            built.scale != transform.scale || built.settings != self.asking(asked)
        })
    }

    /// What the tool is asking for, with what this model carries laid over it: its own
    /// blockers, and a sleeve of wall around every channel so the cavity keeps off the
    /// pipe. A channel placed on a hollow model therefore makes its shell stale.
    pub fn asking(&self, asked: &HollowSettings) -> HollowSettings {
        let mut blockers = self.blockers.clone();
        blockers.extend(sleeves(&self.channels, asked.thickness_mm));
        HollowSettings {
            blockers,
            ..asked.clone()
        }
    }

    /// Puts a blocker where the model was clicked. `point` is in plate coordinates.
    pub fn add_blocker(&mut self, point: Vec3, radius_mm: Scalar, transform: Transform) {
        let matrix = transform.to_matrix();
        if matrix.determinant().abs() < SINGULAR {
            return;
        }
        // The blocker is kept in the model's own space, so a radius asked for in plate
        // millimetres has to come back through the scale the model stands at.
        let scale = transform.scale.abs();
        let smallest = scale.x.min(scale.y).min(scale.z).max(f32::EPSILON);

        self.blockers.push(Blocker::ball(
            matrix.inverse().transform_point3(point),
            radius_mm / smallest,
        ));
        self.remesh_markers();
    }

    /// Takes away the blocker the click landed in, if any, and says whether it did.
    pub fn remove_blocker(&mut self, point: Vec3, transform: Transform) -> bool {
        let matrix = transform.to_matrix();
        if matrix.determinant().abs() < SINGULAR {
            return false;
        }
        let local = matrix.inverse().transform_point3(point);

        let hit = self
            .blockers
            .iter()
            .position(|blocker| blocker.depth_at(local) >= 0.0);
        if let Some(index) = hit {
            self.blockers.remove(index);
            self.remesh_markers();
            return true;
        }
        false
    }

    pub fn clear_blockers(&mut self) {
        self.blockers.clear();
        self.remesh_markers();
    }

    /// Drills a hole where the model was clicked, straight into the surface it was
    /// clicked on. `point` and `normal` are in plate coordinates.
    pub fn add_drain(
        &mut self,
        mesh: &Mesh,
        bvh: &Bvh,
        point: Vec3,
        normal: Vec3,
        size: HoleSize,
        transform: Transform,
    ) {
        let matrix = transform.to_matrix();
        if matrix.determinant().abs() < SINGULAR {
            return;
        }
        let inverse = matrix.inverse();
        let Some(axis) = inverse.transform_vector3(-normal).try_normalize() else {
            return;
        };

        // A hole is measured on the plate, like a blocker, and kept in the model's own
        // space, so both its numbers come back through the scale the model stands at.
        let scale = transform.scale.abs();
        let smallest = scale.x.min(scale.y).min(scale.z).max(f32::EPSILON);
        let at = inverse.transform_point3(point);
        let diameter_mm = size.diameter_mm / smallest;
        self.drains.push(DrainHole {
            at,
            axis,
            diameter_mm,
            depth_mm: size.depth_mm / smallest,
            taper: size.taper,
            lift_mm: lift_for(mesh, bvh, at, axis, diameter_mm),
        });
        self.recut();
    }

    /// Takes away the hole the click landed in, if any, and says whether it did.
    pub fn remove_drain(&mut self, point: Vec3, transform: Transform) -> bool {
        let matrix = transform.to_matrix();
        if matrix.determinant().abs() < SINGULAR {
            return false;
        }
        let local = matrix.inverse().transform_point3(point);

        let hit = self.drains.iter().position(|hole| within(hole, local));
        if let Some(index) = hit {
            self.drains.remove(index);
            self.recut();
            return true;
        }
        false
    }

    pub fn clear_drains(&mut self) {
        self.drains.clear();
        self.recut();
    }

    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    /// Points of the channel being laid out with their normals, in the model's own space.
    pub fn pending(&self) -> &[(Vec3, Vec3)] {
        &self.pending
    }

    /// Adds a point to the channel being laid out. `point` and `normal` are in plate
    /// coordinates; the tube itself is sunk under them when the channel is dug.
    pub fn add_channel_point(&mut self, point: Vec3, normal: Vec3, transform: Transform) {
        let matrix = transform.to_matrix();
        if matrix.determinant().abs() < SINGULAR {
            return;
        }
        // A normal is carried the other way round by the transpose, not by the inverse.
        let normal = matrix.transpose().transform_vector3(normal);
        self.pending.push((
            matrix.inverse().transform_point3(point),
            normal.normalize_or_zero(),
        ));
        self.remesh_markers();
    }

    /// Digs the channel laid out so far, if it has two points to run between.
    pub fn finish_channel(&mut self, diameter_mm: Scalar, transform: Transform) -> bool {
        let scale = transform.scale.abs();
        let smallest = scale.x.min(scale.y).min(scale.z).max(f32::EPSILON);
        let dug = channel_under(&self.pending, diameter_mm / smallest);
        self.pending.clear();
        let Some(channel) = dug else {
            self.recut();
            return false;
        };

        self.channels.push(channel);
        self.recut();
        true
    }

    pub fn clear_channels(&mut self) {
        self.channels.clear();
        self.pending.clear();
        self.recut();
    }

    fn remesh_markers(&mut self) {
        let balls = self
            .blockers
            .iter()
            .map(|blocker| (blocker.from, blocker.radius_mm))
            .chain(
                self.pending
                    .iter()
                    .map(|(point, _)| (*point, PENDING_MARKER_MM)),
            );
        self.markers = markers(balls).map(Arc::new);
    }

    /// Re-cuts what is placed. A hole is drilled to the depth it was asked for, and once
    /// there is a cavity behind it, through the wall the cavity left; see ADR 0075.
    fn recut(&mut self) {
        self.deepened = match self.built.as_ref() {
            Some(built) => pierce(&self.drains, built.settings.thickness_mm, built.voxel_mm),
            None => self.drains.clone(),
        };
        // A hole is placed on a surface and a channel dug between points, so the only body
        // that can fail to mesh is one that was never placed.
        self.cuts = drill(&self.deepened, &self.channels)
            .ok()
            .filter(|bodies| !bodies.is_empty())
            .map(Arc::new);
        // The wall a cavity left is what a bore may be drawn on, and nothing past it.
        let wall_mm = self
            .built
            .as_ref()
            .map(|built| built.settings.thickness_mm + 2.0 * built.voxel_mm);
        self.bores = self
            .model
            .as_ref()
            .and_then(|(mesh, bvh)| bores(mesh, bvh, &self.deepened, &self.channels, wall_mm).ok())
            .filter(|walls| !walls.is_empty())
            .map(Arc::new);
        // Digging a channel takes the points it was laid out from, which are markers.
        self.remesh_markers();
    }

    /// Takes what a run built. The blockers and holes it was built with stay as they are:
    /// the run was started from them.
    pub fn take(&mut self, shell: Shell) {
        self.built = Some(shell);
        // A hole now has a wall to go through, so it is re-cut against it.
        self.recut();
    }

    /// Makes the model solid again, keeping the blockers and holes for the next run.
    pub fn clear(&mut self) {
        self.built = None;
        self.recut();
    }
}

/// Whether `point` is inside the tube `hole` cuts, in the model's own space.
fn within(hole: &DrainHole, point: Vec3) -> bool {
    let along = (point - hole.at).dot(hole.axis);
    let radius = hole.diameter_mm / 2.0;
    (-radius..=hole.depth_mm).contains(&along)
        && (point - hole.at - hole.axis * along).length() <= radius
}

/// Balls of the given radii about the given centres, in one mesh, or `None` for none: how
/// a marked point is drawn, so that every marker in a viewport looks the same.
pub fn markers(balls: impl IntoIterator<Item = (Vec3, Scalar)>) -> Option<Mesh> {
    let mut mesh = Mesh::default();
    for (center, radius_mm) in balls {
        append(&mut mesh, &ball(center, radius_mm));
    }
    (!mesh.is_empty()).then_some(mesh)
}

/// A ball of `radius_mm` about `center`, wound outward.
fn ball(center: Vec3, radius_mm: Scalar) -> Mesh {
    let mut vertices = vec![center + Vec3::new(0.0, 0.0, radius_mm)];
    for ring in 1..MARKER_RINGS {
        let theta = std::f32::consts::PI * ring as Scalar / MARKER_RINGS as Scalar;
        let (sin_theta, cos_theta) = theta.sin_cos();
        for segment in 0..MARKER_SEGMENTS {
            let phi = std::f32::consts::TAU * segment as Scalar / MARKER_SEGMENTS as Scalar;
            let (sin_phi, cos_phi) = phi.sin_cos();
            vertices.push(
                center + radius_mm * Vec3::new(sin_theta * cos_phi, sin_theta * sin_phi, cos_theta),
            );
        }
    }
    let south = vertices.len() as u32;
    vertices.push(center - Vec3::new(0.0, 0.0, radius_mm));

    let at = |ring: usize, segment: usize| {
        (1 + (ring - 1) * MARKER_SEGMENTS + segment % MARKER_SEGMENTS) as u32
    };
    let mut faces = Vec::new();
    for segment in 0..MARKER_SEGMENTS {
        faces.push([0, at(1, segment), at(1, segment + 1)]);
    }
    for ring in 1..MARKER_RINGS - 1 {
        for segment in 0..MARKER_SEGMENTS {
            faces.push([
                at(ring, segment),
                at(ring + 1, segment),
                at(ring + 1, segment + 1),
            ]);
            faces.push([
                at(ring, segment),
                at(ring + 1, segment + 1),
                at(ring, segment + 1),
            ]);
        }
    }
    for segment in 0..MARKER_SEGMENTS {
        faces.push([
            at(MARKER_RINGS - 1, segment),
            south,
            at(MARKER_RINGS - 1, segment + 1),
        ]);
    }
    Mesh::new(vertices, faces)
}

fn append(whole: &mut Mesh, part: &Mesh) {
    let offset = whole.vertices.len() as u32;
    whole.vertices.extend_from_slice(&part.vertices);
    whole.faces.extend(
        part.faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Quat, signed_volume};
    use std::f32::consts::FRAC_PI_2;

    fn drain_tool(diameter_mm: Scalar, depth_mm: Scalar) -> HoleSize {
        HoleSize {
            diameter_mm,
            depth_mm,
            taper: 1.0,
        }
    }

    /// A flat lid in the z = 0 plane, wide enough to drill anywhere in these tests: a
    /// surface that rises nowhere, so a hole on it needs no lift.
    fn lid() -> Mesh {
        Mesh::new(
            vec![
                Vec3::new(-50.0, -50.0, 0.0),
                Vec3::new(50.0, -50.0, 0.0),
                Vec3::new(50.0, 50.0, 0.0),
                Vec3::new(-50.0, 50.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        )
    }

    fn shell(scale: Vec3, settings: HollowSettings) -> Shell {
        Shell {
            mesh: Arc::new(Mesh::default()),
            cavity_mm3: 1234.0,
            voxel_mm: 0.2,
            coarsened: false,
            scale,
            settings,
        }
    }

    #[test]
    fn a_new_object_is_solid() {
        let hollow = ModelHollow::default();
        assert!(!hollow.is_hollow());
        assert!(hollow.shell().is_none());
        assert!(hollow.cavity_mm3().abs() < f32::EPSILON);
    }

    #[test]
    fn a_blocker_is_stored_in_the_models_own_space() {
        let transform = Transform::from_translation(Vec3::new(50.0, 0.0, 0.0));
        let mut hollow = ModelHollow::default();
        hollow.add_blocker(Vec3::new(60.0, 1.0, 2.0), 4.0, transform);

        let blocker = hollow.blockers()[0];
        assert!(blocker.from.abs_diff_eq(Vec3::new(10.0, 1.0, 2.0), 1e-4));
        assert!((blocker.radius_mm - 4.0).abs() < 1e-4);
    }

    #[test]
    fn a_blocker_on_a_scaled_model_keeps_the_size_it_was_asked_for() {
        let transform = Transform {
            scale: Vec3::splat(2.0),
            ..Transform::default()
        };
        let mut hollow = ModelHollow::default();
        hollow.add_blocker(Vec3::new(4.0, 0.0, 0.0), 4.0, transform);

        // Two millimetres in the model's own space is four on a plate it is doubled on.
        let blocker = hollow.blockers()[0];
        assert!((blocker.radius_mm - 2.0).abs() < 1e-4);
        assert!(blocker.from.abs_diff_eq(Vec3::new(2.0, 0.0, 0.0), 1e-4));
    }

    #[test]
    fn a_blocker_travels_with_the_model_it_was_put_on() {
        let transform = Transform {
            rotation: Quat::from_rotation_z(FRAC_PI_2),
            ..Transform::default()
        };
        let mut hollow = ModelHollow::default();
        hollow.add_blocker(Vec3::new(-10.0, 10.0, 0.0), 4.0, transform);

        // A quarter turn about z maps the model's own (10, 10) onto the plate's (-10, 10).
        assert!(
            hollow.blockers()[0]
                .from
                .abs_diff_eq(Vec3::new(10.0, 10.0, 0.0), 1e-4)
        );
    }

    #[test]
    fn clicking_a_blocker_takes_it_away() {
        let mut hollow = ModelHollow::default();
        hollow.add_blocker(Vec3::ZERO, 4.0, Transform::default());

        assert!(!hollow.remove_blocker(Vec3::new(20.0, 0.0, 0.0), Transform::default()));
        assert!(hollow.remove_blocker(Vec3::new(1.0, 0.0, 0.0), Transform::default()));
        assert!(hollow.blockers().is_empty());
        assert!(
            hollow.markers().is_none(),
            "the marker goes with the blocker"
        );
    }

    #[test]
    fn a_blocker_is_drawn_as_a_ball_where_it_stands() {
        let mut hollow = ModelHollow::default();
        hollow.add_blocker(Vec3::new(3.0, 0.0, 0.0), 2.0, Transform::default());

        let markers = hollow.markers().expect("a blocker is drawn").clone();
        let bounds = markers.aabb().expect("the marker has vertices");
        assert!(bounds.mins.abs_diff_eq(Vec3::new(1.0, -2.0, -2.0), 1e-4));
        assert!(bounds.maxs.abs_diff_eq(Vec3::new(5.0, 2.0, 2.0), 1e-4));
    }

    #[test]
    fn a_drain_hole_is_stored_in_the_models_own_space_and_points_into_it() {
        let transform = Transform::from_translation(Vec3::new(50.0, 0.0, 0.0));
        let mut hollow = ModelHollow::default();
        hollow.add_drain(
            &lid(),
            &Bvh::build(&lid()),
            Vec3::new(60.0, 0.0, 10.0),
            Vec3::Z,
            drain_tool(3.0, 4.0),
            transform,
        );

        let hole = hollow.drains()[0];
        assert!(hole.at.abs_diff_eq(Vec3::new(10.0, 0.0, 10.0), 1e-4));
        assert!(
            hole.axis.abs_diff_eq(Vec3::NEG_Z, 1e-4),
            "a hole is drilled the other way from the face it was clicked on, got {}",
            hole.axis
        );
    }

    #[test]
    fn a_drain_hole_on_a_scaled_model_keeps_the_size_it_was_asked_for() {
        let transform = Transform {
            scale: Vec3::splat(2.0),
            ..Transform::default()
        };
        let mut hollow = ModelHollow::default();
        hollow.add_drain(
            &lid(),
            &Bvh::build(&lid()),
            Vec3::new(0.0, 0.0, 4.0),
            Vec3::Z,
            drain_tool(3.0, 4.0),
            transform,
        );

        let hole = hollow.drains()[0];
        assert!((hole.diameter_mm - 1.5).abs() < 1e-4);
        assert!((hole.depth_mm - 2.0).abs() < 1e-4);
    }

    #[test]
    fn clicking_a_drain_hole_takes_it_away() {
        let mut hollow = ModelHollow::default();
        hollow.add_drain(
            &lid(),
            &Bvh::build(&lid()),
            Vec3::ZERO,
            Vec3::Z,
            drain_tool(4.0, 5.0),
            Transform::default(),
        );

        assert!(!hollow.remove_drain(Vec3::new(9.0, 0.0, 0.0), Transform::default()));
        assert!(hollow.remove_drain(Vec3::new(0.0, 0.0, -2.0), Transform::default()));
        assert!(hollow.drains().is_empty());
        assert!(hollow.cut_bodies().is_none(), "the tube goes with the hole");
    }

    #[test]
    fn a_drain_hole_is_cut_as_the_tube_it_takes_out() {
        let mut hollow = ModelHollow::default();
        hollow.add_drain(
            &lid(),
            &Bvh::build(&lid()),
            Vec3::ZERO,
            Vec3::Z,
            drain_tool(4.0, 5.0),
            Transform::default(),
        );

        let bodies = hollow.cut_bodies().expect("a hole is a body").clone();
        let bounds = bodies.aabb().expect("the tube has vertices");
        assert!(
            bounds.maxs.z > 0.0 && bounds.mins.z <= -5.0,
            "the tube runs from just clear of the lid down to its own depth, got {bounds:?}"
        );
    }

    #[test]
    fn a_channel_is_dug_only_once_it_has_two_points_to_run_between() {
        let mut hollow = ModelHollow::default();
        hollow.add_channel_point(Vec3::new(1.0, 0.0, 0.0), Vec3::X, Transform::default());
        assert!(
            !hollow.finish_channel(2.0, Transform::default()),
            "one point is not a channel"
        );
        assert!(hollow.channels().is_empty());

        hollow.add_channel_point(Vec3::new(1.0, 0.0, 0.0), Vec3::X, Transform::default());
        hollow.add_channel_point(Vec3::new(1.0, 0.0, 6.0), Vec3::X, Transform::default());
        assert!(hollow.finish_channel(2.0, Transform::default()));

        let channel = &hollow.channels()[0];
        assert!((channel.diameter_mm - 2.0).abs() < 1e-4);
        assert_eq!(
            channel.points.len(),
            4,
            "a mouth at each end over the dive under either click"
        );
        for under in &channel.points[1..3] {
            assert!(
                under.x <= 1.0 - 1.0,
                "the run is sunk clear of the face it was drawn on, got {under}"
            );
        }
        assert!(
            hollow.pending().is_empty(),
            "the points go into the channel that was dug from them"
        );
    }

    #[test]
    fn a_hole_is_cut_the_moment_it_is_placed_and_deepened_once_there_is_a_wall() {
        let mut hollow = ModelHollow::default();
        hollow.add_drain(
            &lid(),
            &Bvh::build(&lid()),
            Vec3::ZERO,
            Vec3::Z,
            drain_tool(3.0, 0.5),
            Transform::default(),
        );

        assert!(
            (hollow.cut().0[0].depth_mm - 0.5).abs() < 1e-6,
            "a hole on a solid model is cut to the depth it was asked for"
        );
        let bodies = hollow
            .cut_bodies()
            .expect("a placed hole is a body")
            .clone();
        assert!(
            signed_volume(&bodies) < 0.0,
            "the body is wound inward, which is what subtracts it"
        );
        assert!(
            hollow.markers().is_none(),
            "a hole is geometry from the first click, never a marker"
        );

        // Two millimetres of wall on a 0.2 mm lattice, which a half-millimetre dimple
        // cannot reach through.
        hollow.take(shell(Vec3::ONE, hollow.asking(&HollowSettings::default())));
        assert!(
            hollow.cut().0[0].depth_mm >= 2.0,
            "once there is a cavity the hole is re-cut through the wall, got {}",
            hollow.cut().0[0].depth_mm
        );
    }

    #[test]
    fn drilling_a_hole_leaves_the_shell_alone() {
        let asked = HollowSettings::default();
        let mut hollow = ModelHollow::default();
        hollow.take(shell(Vec3::ONE, asked.clone()));
        hollow.add_drain(
            &lid(),
            &Bvh::build(&lid()),
            Vec3::ZERO,
            Vec3::Z,
            drain_tool(3.0, 4.0),
            Transform::default(),
        );

        assert!(
            !hollow.is_stale(&asked, Transform::default()),
            "a hole is cut on its own, so the cavity does not have to be built again"
        );
    }

    #[test]
    fn a_shell_built_from_the_same_numbers_is_not_stale() {
        let asked = HollowSettings::default();
        let mut hollow = ModelHollow::default();
        hollow.take(shell(Vec3::ONE, asked.clone()));

        assert!(!hollow.is_stale(&asked, Transform::default()));
    }

    #[test]
    fn changing_the_wall_makes_the_shell_stale() {
        let mut hollow = ModelHollow::default();
        hollow.take(shell(Vec3::ONE, HollowSettings::default()));

        let thicker = HollowSettings {
            thickness_mm: 3.0,
            ..HollowSettings::default()
        };
        assert!(hollow.is_stale(&thicker, Transform::default()));
    }

    #[test]
    fn adding_a_blocker_makes_the_shell_stale() {
        let asked = HollowSettings::default();
        let mut hollow = ModelHollow::default();
        hollow.take(shell(Vec3::ONE, asked.clone()));
        hollow.add_blocker(Vec3::ZERO, 4.0, Transform::default());

        assert!(hollow.is_stale(&asked, Transform::default()));
    }

    #[test]
    fn digging_a_channel_makes_the_shell_stale_and_asks_for_a_sleeve_of_wall() {
        let asked = HollowSettings::default();
        let mut hollow = ModelHollow::default();
        hollow.take(shell(Vec3::ONE, asked.clone()));
        hollow.add_channel_point(Vec3::new(1.0, 0.0, 0.0), Vec3::X, Transform::default());
        hollow.add_channel_point(Vec3::new(1.0, 0.0, 6.0), Vec3::X, Transform::default());
        hollow.finish_channel(2.0, Transform::default());

        assert!(
            hollow.is_stale(&asked, Transform::default()),
            "the cavity has to be cut again to keep off the pipe"
        );
        let wanted = hollow.asking(&asked);
        assert_eq!(
            wanted.blockers.len(),
            3,
            "a sleeve over each leg of the spine"
        );
        assert!(
            wanted
                .blockers
                .iter()
                .all(|blocker| { (blocker.radius_mm - (1.0 + asked.thickness_mm)).abs() < 1e-4 }),
            "the sleeve is the tube's radius and the wall around it"
        );
    }

    #[test]
    fn rescaling_the_model_makes_the_shell_stale() {
        let asked = HollowSettings::default();
        let mut hollow = ModelHollow::default();
        hollow.take(shell(Vec3::ONE, asked.clone()));

        let doubled = Transform {
            scale: Vec3::splat(2.0),
            ..Transform::default()
        };
        assert!(
            hollow.is_stale(&asked, doubled),
            "a wall measured in the model's own space is the wrong thickness once the \
             model is scaled"
        );
    }
}

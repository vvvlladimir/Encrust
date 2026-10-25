use std::ops::Range;
use std::sync::{Arc, Weak};

use core_geometry::{Bvh, Mat3, Mesh, Scalar, Transform, Vec3, smallest_scale};

use crate::{
    Blocker, Channel, DrainHole, Drilled, HoleSize, HollowSettings, channel_under, drill, hole_at,
    lift_for, pierce, sleeves, turn_in_model,
};

/// How wide a point of a channel still being laid out is drawn, in millimetres.
pub const PENDING_MARKER_MM: Scalar = 0.6;

/// Rings and segments of the ball a marker is drawn as: fine enough to stay round when the
/// camera is close and a cut shows its outline, under a thousand faces a ball.
const MARKER_RINGS: usize = 16;
const MARKER_SEGMENTS: usize = 24;

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
    built: Option<Shell>,
    /// The cuts taken out of the model exactly, rebuilt off the drawing thread whenever they
    /// or what they cut change, and never kept in an undo snapshot (ADR 0322).
    drilled: Option<DrilledFrom>,
}

/// What taking the cuts out exactly gave, `None` where the engine refused the model, and the
/// meshes it was given: held weakly, so a replaced one is let go and no longer matches.
#[derive(Debug, Clone)]
struct DrilledFrom {
    source: Weak<Mesh>,
    cuts: Weak<Mesh>,
    drilled: Option<Drilled>,
}

/// Whether two shapes are one, but for the rounding a turn leaves in them.
fn same_shape(built: Mat3, now: Mat3) -> bool {
    let size = built
        .abs()
        .to_cols_array()
        .into_iter()
        .fold(1.0, Scalar::max);
    built.abs_diff_eq(now, SHAPE_TOLERANCE * size)
}

/// Relative to the shape's largest entry: far under a hundredth of a percent of a resize.
const SHAPE_TOLERANCE: Scalar = 1e-5;

/// A model hollowed, in its own space: what a run of `hollow` hands back to be kept.
#[derive(Debug, Clone, PartialEq)]
pub struct Shell {
    /// The outer surface, the cavity wound inward and its infill.
    pub mesh: Arc<Mesh>,
    /// Which faces of `mesh` bound the space the resin fills; see [`crate::Hollowed`].
    pub cavity: Range<usize>,
    pub cavity_mm3: Scalar,
    /// The lattice the cavity came out on, millimetres, and whether the memory budget
    /// made it coarser than the precision asked for.
    pub voxel_mm: Scalar,
    pub coarsened: bool,
    /// The placement the wall was measured under, with its turn taken out
    /// (`Transform::unturned`).
    pub shape: Mat3,
    pub settings: HollowSettings,
}

impl ModelHollow {
    /// The cuts a project file was saved with. The cavity itself is not in the file: it is
    /// asked for again by the Hollow tool.
    pub fn restored(
        blockers: Vec<Blocker>,
        drains: Vec<DrainHole>,
        channels: Vec<Channel>,
    ) -> Self {
        let mut hollow = Self {
            blockers,
            drains,
            channels,
            ..Self::default()
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

    /// The model with its cuts taken out exactly, for `source`, the mesh it prints from — its
    /// shell once hollowed —, or `None` while they are being taken out, where the engine
    /// refused the model, or with nothing cut.
    pub fn drilled(&self, source: &Arc<Mesh>) -> Option<&Drilled> {
        self.drilled_for(source)?.drilled.as_ref()
    }

    /// Whether the cuts still wait to be taken out of `source` exactly: nothing has been tried
    /// for them and it yet.
    pub fn wants_drilling(&self, source: &Arc<Mesh>) -> bool {
        self.cuts.is_some() && self.drilled_for(source).is_none()
    }

    /// Keeps what taking `cuts` out of `source` exactly gave, `None` where the engine refused
    /// it; dropped once the model's cuts are no longer `cuts`.
    pub fn keep_drilled(&mut self, source: &Arc<Mesh>, cuts: &Arc<Mesh>, drilled: Option<Drilled>) {
        if self.cuts.as_ref().is_some_and(|now| Arc::ptr_eq(now, cuts)) {
            self.drilled = Some(DrilledFrom {
                source: Arc::downgrade(source),
                cuts: Arc::downgrade(cuts),
                drilled,
            });
        }
    }

    /// Lets go of the cuts taken out exactly, as an undo snapshot does: they are taken out
    /// again for the plate put back.
    pub fn forget_drilled(&mut self) {
        self.drilled = None;
    }

    /// Takes over what `other` drilled where it was drilled for this model's own cuts, as
    /// an undo that leaves them alone does.
    pub fn adopt_drilled(&mut self, other: &Self) {
        let same = match (&self.cuts, &other.cuts) {
            (Some(mine), Some(theirs)) => Arc::ptr_eq(mine, theirs),
            _ => false,
        };
        if same && self.drilled.is_none() {
            self.drilled.clone_from(&other.drilled);
        }
    }

    fn drilled_for(&self, source: &Arc<Mesh>) -> Option<&DrilledFrom> {
        let cuts = self.cuts.as_ref()?;
        self.drilled.as_ref().filter(|from| {
            from.source.as_ptr() == Arc::as_ptr(source) && from.cuts.as_ptr() == Arc::as_ptr(cuts)
        })
    }

    /// Drills a hole from the nearest surface into `target`, a point in the model's own
    /// space, deep enough to reach it: what gives a trapped pocket its way out.
    ///
    /// `size` is measured on the plate, like the one a click drops; its depth is the
    /// pocket's. Returns whether there was a surface to drill through at all.
    pub fn drill_into(
        &mut self,
        mesh: &Mesh,
        bvh: &Bvh,
        target: Vec3,
        size: HoleSize,
        transform: Transform,
    ) -> bool {
        let smallest = transform.least_scale();
        let diameter_mm = size.diameter_mm / smallest;
        let asked = HoleSize {
            diameter_mm,
            depth_mm: 0.0,
            ..size
        };
        let Some(hole) = hole_at(mesh, bvh, target, asked) else {
            return false;
        };
        let turn_deg = turn_in_model(hole.axis, size.turn_deg, transform.linear());

        // A diameter past the pocket's floor, so the tip is inside the pocket rather than
        // on its ceiling.
        let depth_mm = (target - hole.at).length() + diameter_mm;
        self.drains.push(DrainHole {
            depth_mm,
            turn_deg,
            lift_mm: lift_for(mesh, bvh, hole.at, hole.axis, diameter_mm, depth_mm),
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

    /// Which faces of [`Self::shell`] bound the space the resin fills, or `None` while the
    /// model is still solid. It is what a viewport paints to show resin with no way out.
    pub fn cavity_faces(&self) -> Option<Range<usize>> {
        self.built.as_ref().map(|built| built.cavity.clone())
    }

    /// What the standing shell was built from, or `None` while the model is still solid.
    pub fn asked(&self) -> Option<&HollowSettings> {
        self.built.as_ref().map(|built| &built.settings)
    }

    /// Resin the cavity takes out of this model, in cubic millimetres.
    pub fn cavity_mm3(&self) -> Scalar {
        self.built.as_ref().map_or(0.0, |built| built.cavity_mm3)
    }

    /// The placement the standing shell's wall was measured under, or `None` while the
    /// model is still solid; see [`Shell::shape`].
    pub fn built_shape(&self) -> Option<Mat3> {
        self.built.as_ref().map(|built| built.shape)
    }

    /// The lattice the cavity was cut on, and whether the memory budget coarsened it.
    pub fn lattice(&self) -> Option<(Scalar, bool)> {
        self.built
            .as_ref()
            .map(|built| (built.voxel_mm, built.coarsened))
    }

    /// Whether what is built no longer matches what the tool is asking for, or the model
    /// has been rescaled or stretched under it. A wall is measured on the plate under the
    /// shape it was built for, so a model resized after it was hollowed has a wall of the
    /// wrong thickness. A turn alone changes nothing.
    pub fn is_stale(&self, asked: &HollowSettings, transform: Transform) -> bool {
        self.built.as_ref().is_some_and(|built| {
            !same_shape(built.shape, transform.unturned()) || built.settings != self.asking(asked)
        })
    }

    /// What the standing shell would be built from now, at the numbers it was built at, or
    /// `None` when it already is that or the model is solid. A channel dug or cleared, or a
    /// blocker placed, after the model was hollowed is what moves it.
    pub fn rebuild(&self) -> Option<HollowSettings> {
        let built = self.asked()?;
        let wanted = self.asking(built);
        (wanted != *built).then_some(wanted)
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
        if let Some(blocker) = Self::blocker_at(point, radius_mm, transform) {
            self.blockers.push(blocker);
            self.remesh_markers();
        }
    }

    /// The blocker a click at `point`, in plate coordinates, would put down, in the
    /// model's own space, or `None` where the placement has flattened the model.
    pub fn blocker_at(point: Vec3, radius_mm: Scalar, transform: Transform) -> Option<Blocker> {
        let inverse = transform.inverse_matrix()?;
        // The blocker is kept in the model's own space, so a radius asked for in plate
        // millimetres has to come back through the scale the model stands at.
        let smallest = transform.least_scale();
        Some(Blocker::ball(
            inverse.transform_point3(point),
            radius_mm / smallest,
        ))
    }

    /// Takes away the blocker the click landed in, if any, and says whether it did.
    pub fn remove_blocker(&mut self, point: Vec3, transform: Transform) -> bool {
        let Some(inverse) = transform.inverse_matrix() else {
            return false;
        };
        let local = inverse.transform_point3(point);

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
    /// clicked on, unless one already drilled there takes it in. `point` and `normal` are
    /// in plate coordinates.
    pub fn add_drain(
        &mut self,
        mesh: &Mesh,
        bvh: &Bvh,
        point: Vec3,
        normal: Vec3,
        size: HoleSize,
        transform: Transform,
    ) {
        let hole = Self::drain_at(mesh, bvh, point, normal, size, transform);
        if let Some(hole) = hole.filter(|hole| !self.has_hole_over(hole)) {
            self.drains.push(hole);
            self.recut();
        }
    }

    /// Whether `hole`, in the model's own space, would land in one already drilled at least
    /// as wide: a click on a hole adds nothing, a wider one over it is drilled.
    pub fn has_hole_over(&self, hole: &DrainHole) -> bool {
        self.drains
            .iter()
            .any(|old| within(old, hole.at) && hole.diameter_mm <= old.diameter_mm)
    }

    /// The hole a click at `point` on a face of `normal`, both in plate coordinates, would
    /// drill, in the model's own space, or `None` where it cannot be drilled.
    pub fn drain_at(
        mesh: &Mesh,
        bvh: &Bvh,
        point: Vec3,
        normal: Vec3,
        size: HoleSize,
        transform: Transform,
    ) -> Option<DrainHole> {
        let inverse = transform.inverse_matrix()?;
        let axis = inverse.transform_vector3(-normal).try_normalize()?;

        // A hole is measured on the plate, like a blocker, and kept in the model's own
        // space, so both its numbers come back through the scale the model stands at.
        let smallest = transform.least_scale();
        let at = inverse.transform_point3(point);
        let size = HoleSize {
            diameter_mm: size.diameter_mm / smallest,
            depth_mm: size.depth_mm / smallest,
            turn_deg: turn_in_model(axis, size.turn_deg, transform.linear()),
            ..size
        };
        let lift_mm = lift_for(mesh, bvh, at, axis, size.diameter_mm, size.depth_mm);
        Some(DrainHole::sized(at, axis, size, lift_mm))
    }

    /// `hole` as it would be cut here: through the wall, once the model is hollow.
    pub fn as_cut(&self, hole: DrainHole) -> DrainHole {
        self.pierced(std::slice::from_ref(&hole))
            .pop()
            .unwrap_or(hole)
    }

    /// Takes away the hole the click landed in, if any, and says whether it did.
    pub fn remove_drain(&mut self, point: Vec3, transform: Transform) -> bool {
        let Some(inverse) = transform.inverse_matrix() else {
            return false;
        };
        let local = inverse.transform_point3(point);

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
        if let Some(pending) = Self::channel_point_at(point, normal, transform) {
            self.pending.push(pending);
            self.remesh_markers();
        }
    }

    /// The point and outward normal a click would add to a channel, both in plate
    /// coordinates, carried into the model's own space.
    pub fn channel_point_at(
        point: Vec3,
        normal: Vec3,
        transform: Transform,
    ) -> Option<(Vec3, Vec3)> {
        let matrix = transform.to_matrix();
        let inverse = transform.inverse_matrix()?;
        // A normal is carried the other way round by the transpose, not by the inverse.
        let normal = matrix.transpose().transform_vector3(normal);
        Some((inverse.transform_point3(point), normal.normalize_or_zero()))
    }

    /// Digs the channel laid out so far, if it has two points to run between, as wide and
    /// in the outline `size` asks for; its depth plays no part.
    pub fn finish_channel(&mut self, size: HoleSize, transform: Transform) -> bool {
        let smallest = transform.least_scale();
        let dug = channel_under(&self.pending, size.diameter_mm / smallest)
            .map(|channel| Channel {
                shape: size.shape,
                turn_deg: turn_in_model(
                    channel.points[1] - channel.points[0],
                    size.turn_deg,
                    transform.linear(),
                ),
                ..channel
            })
            .filter(|channel| drill(&[], std::slice::from_ref(channel)).is_ok());
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
        self.deepened = self.pierced(&self.drains);
        // A hole is placed on a surface and a channel dug between points, so the only body
        // that can fail to mesh is one that was never placed.
        self.cuts = drill(&self.deepened, &self.channels)
            .ok()
            .filter(|bodies| !bodies.is_empty())
            .map(Arc::new);
        // Digging a channel takes the points it was laid out from, which are markers.
        self.remesh_markers();
    }

    fn pierced(&self, holes: &[DrainHole]) -> Vec<DrainHole> {
        match self.built.as_ref() {
            Some(built) => pierce(holes, built.settings.thickness_mm, built.voxel_mm),
            None => holes.to_vec(),
        }
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

    /// These blockers, holes and channels carried onto `mesh`: the model's surface remade
    /// with `sizing` pressed into it, as a relief does. Solid, for the next run to hollow.
    #[must_use]
    pub fn carried_onto(&self, mesh: &Mesh, bvh: &Bvh, sizing: Mat3) -> Self {
        // Sizes were divided by the least scale on the way in (`blocker_at`, `drain_at`).
        let least = smallest_scale(sizing);
        let blockers = self
            .blockers
            .iter()
            .map(|blocker| Blocker {
                from: sizing * blocker.from,
                to: sizing * blocker.to,
                radius_mm: blocker.radius_mm * least,
            })
            .collect();
        let drains = self
            .drains
            .iter()
            .map(|hole| {
                let at = sizing * hole.at;
                let axis = (sizing * hole.axis).try_normalize().unwrap_or(hole.axis);
                let diameter_mm = hole.diameter_mm * least;
                let depth_mm = hole.depth_mm * least;
                DrainHole {
                    at,
                    axis,
                    diameter_mm,
                    depth_mm,
                    lift_mm: lift_for(mesh, bvh, at, axis, diameter_mm, depth_mm),
                    ..*hole
                }
            })
            .collect();
        let channels = self
            .channels
            .iter()
            .map(|channel| Channel {
                points: channel.points.iter().map(|point| sizing * *point).collect(),
                diameter_mm: channel.diameter_mm * least,
                ..*channel
            })
            .collect();
        let mut carried = Self {
            blockers,
            drains,
            channels,
            ..Self::default()
        };
        carried.recut();
        carried
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
mod tests;

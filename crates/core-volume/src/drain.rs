use core_geometry::{Bvh, Mat3, Mesh, Ray, Scalar, Vec3};

use serde::{Deserialize, Serialize};

use crate::error::VolumeError;
use channel::tunnel;

mod channel;

pub use channel::Leg;

/// Sides of a hole's tube. A three millimetre hole is then within thirteen micrometres of
/// round, which is under half a pixel on any panel the workspace writes for.
const SIDES: usize = 24;

/// Points around a mouth's rim that are asked how far the surface rises over it.
const RIM_SAMPLES: usize = 12;

/// How far past the surface a mouth always starts, millimetres.
///
/// Public because a channel's ends have to clear the surface the same way.
///
/// A cap sitting exactly on the surface leaves whatever the facets of that surface are off
/// the plane by, which prints as a film over the hole. Half a layer of the finest machine
/// is enough to clear it and too little to show on the model.
pub const MOUTH_LIFT_MM: Scalar = 0.05;

/// How many times a cut is laid over itself. Its winding has to outweigh every body that
/// can stand where it lands — a shell, the lattice bonded into it, three struts meeting at
/// a corner, a support's tip — or what they add up to is left in the hole; see ADR 0188.
pub const CUT_WEIGHT: usize = 8;

/// The outline a hole is cut to across its axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HoleShape {
    #[default]
    Round,
    /// A regular polygon of this many sides, its corners on the hole's diameter.
    Polygon { sides: u8 },
}

impl HoleShape {
    /// The fewest and the most sides a polygon is cut with; past twelve it reads as round.
    pub const MIN_SIDES: u8 = 3;
    pub const MAX_SIDES: u8 = 12;

    /// How many sides the hole is cut with: a round one as finely as a channel.
    pub fn sides(self) -> usize {
        match self {
            Self::Round => SIDES,
            Self::Polygon { sides } => usize::from(sides.clamp(Self::MIN_SIDES, Self::MAX_SIDES)),
        }
    }

    /// How many corners a polygon has, or `None` for a round hole, drawn as a true circle.
    pub fn corners(self) -> Option<usize> {
        match self {
            Self::Round => None,
            Self::Polygon { .. } => Some(self.sides()),
        }
    }
}

/// The hole a click drills: its mouth and depth in millimetres of the plate, its far end
/// as a fraction of its mouth, 1 being a cylinder, and its outline and turn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoleSize {
    pub diameter_mm: Scalar,
    pub depth_mm: Scalar,
    pub taper: Scalar,
    pub shape: HoleShape,
    /// How far a polygon is turned about the axis, degrees.
    pub turn_deg: Scalar,
}

impl HoleSize {
    /// A round hole, as every hole was before ADR 0313.
    pub fn round(diameter_mm: Scalar, depth_mm: Scalar, taper: Scalar) -> Self {
        Self {
            diameter_mm,
            depth_mm,
            taper,
            shape: HoleShape::Round,
            turn_deg: 0.0,
        }
    }
}

/// A hole drilled into the model so the resin behind it can get out.
///
/// It is subtracted from the whole solid rather than from the cavity alone, so one hole
/// cuts the shell and the cavity's ceiling in the same pass; see
/// `docs/design/hollowing.md`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DrainHole {
    /// Where the mouth sits on the surface, in the model's own space.
    pub at: Vec3,
    /// Direction the hole is drilled in, pointing into the model. Need not be unit.
    pub axis: Vec3,
    /// Diameter of the mouth, millimetres.
    pub diameter_mm: Scalar,
    /// How far past the mouth the hole reaches, millimetres.
    pub depth_mm: Scalar,
    /// Diameter of the far end as a fraction of the mouth: 1 is a cylinder, less a cone.
    pub taper: Scalar,
    /// How far the mouth stands clear of the surface, millimetres.
    ///
    /// What the surface rises around the mouth, from `lift_for`; the mouth always stands
    /// clear by a little more than that, so nothing of it is left capped.
    pub lift_mm: Scalar,
    /// The outline, and how far it is turned about the axis in degrees. A file without
    /// them reads a round hole (ADR 0313).
    #[serde(default)]
    pub shape: HoleShape,
    #[serde(default)]
    pub turn_deg: Scalar,
    /// What the hole is for, which says how hollowing and plugs treat it (ADR 0346, 0347).
    #[serde(default)]
    pub kind: HoleKind,
}

/// What a hole is for. A file without it reads a drain, which every hole was before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HoleKind {
    /// Lets the resin out of a cavity: carried on through the wall, closed by a plug.
    #[default]
    Drain,
    /// Drilled into the solid model before it was hollowed: part of its shape, so the cavity
    /// keeps a wall around it and it keeps its depth and its plug.
    Walled,
    /// Drilled into the supports or raft under the model: no plug, no wall, never deepened.
    Support,
}

impl DrainHole {
    /// A hole of `size` with its mouth at `at`, drilled along `axis`.
    pub fn sized(at: Vec3, axis: Vec3, size: HoleSize, lift_mm: Scalar) -> Self {
        Self {
            at,
            axis,
            diameter_mm: size.diameter_mm,
            depth_mm: size.depth_mm,
            taper: size.taper,
            lift_mm,
            shape: size.shape,
            turn_deg: size.turn_deg,
            kind: HoleKind::Drain,
        }
    }

    /// The unit offset from the axis to the outline's first corner, in the model's own
    /// space, and how many corners there are; `None` for a round hole or no axis.
    pub fn corner(&self) -> Option<(Vec3, usize)> {
        Section::of(self.shape, self.turn_deg).first_corner(self.shape, self.axis)
    }
}

/// A drainage channel: a tube of `diameter_mm` dug along a polyline in model space, round
/// or a polygon turned `turn_deg` from the level about each leg. A file without the outline
/// reads a round one (ADR 0313).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub points: Vec<Vec3>,
    pub diameter_mm: Scalar,
    #[serde(default)]
    pub shape: HoleShape,
    #[serde(default)]
    pub turn_deg: Scalar,
}

/// The bodies `holes` and `channels` take out of a model, wound inward and laid
/// [`CUT_WEIGHT`] deep: appending them to the model is what subtracts them (ADR 0071, 0075,
/// 0188).
///
/// They are meshed, never voxelised, so a hole is exact at any layer height and needs no
/// field of its own. A hole with a diameter, a depth or an axis of nothing cannot be
/// drilled and comes back as an error.
pub fn drill(holes: &[DrainHole], channels: &[Channel]) -> Result<Mesh, VolumeError> {
    let mut cut = Mesh::default();
    for (index, hole) in holes.iter().enumerate() {
        append(
            &mut cut,
            &body_of(hole).ok_or(VolumeError::BadDrain { index })?,
        );
    }
    for (index, channel) in channels.iter().enumerate() {
        append(
            &mut cut,
            &tunnel(channel).ok_or(VolumeError::BadChannel { index })?,
        );
    }
    Ok(weighted(&inward(&cut)))
}

/// `body` laid [`CUT_WEIGHT`] times over itself, each copy closed on vertices of its own so
/// the slicer links every copy into rings of its own.
fn weighted(body: &Mesh) -> Mesh {
    let mut whole = Mesh::default();
    for _ in 0..CUT_WEIGHT {
        append(&mut whole, body);
    }
    whole
}

/// Every drain deepened until it is through a wall of `thickness_mm` cut on a lattice of
/// `voxel_mm`; other holes keep their depth.
///
/// A hole is drilled along the surface's own normal, so the wall in front of it is the
/// thickness the cavity was cut to; a hole left shorter than that is a dimple, not a
/// drain. Two voxels past it clears the cavity surface, which marching cubes puts within
/// half a voxel of where the wall ends.
pub fn pierce(holes: &[DrainHole], thickness_mm: Scalar, voxel_mm: Scalar) -> Vec<DrainHole> {
    holes
        .iter()
        .map(|hole| DrainHole {
            depth_mm: if hole.kind == HoleKind::Drain {
                hole.depth_mm
                    .max(hole.lift_mm + thickness_mm + 2.0 * voxel_mm)
            } else {
                hole.depth_mm
            },
            ..*hole
        })
        .collect()
}

/// The same surface turned the other way round, which is what makes it subtract.
fn inward(mesh: &Mesh) -> Mesh {
    Mesh::new(
        mesh.vertices.clone(),
        mesh.faces.iter().map(|[a, b, c]| [*a, *c, *b]).collect(),
    )
}

/// How far the mouth of a hole of `diameter_mm` at `at` has to stand clear of the
/// surface: the most the surface rises above the plane of the mouth around its own rim,
/// which is nothing where that surface is flat or curves away.
///
/// Each of `RIM_SAMPLES` points around the rim casts a ray out of the model along the
/// hole's axis, and counts only when the first face it meets is one it leaves the material
/// through: whatever stands clear over the mouth, past air, is another part of the model.
/// So is anything rising further than the hole's `depth_mm`: a mouth in a fold is not lifted
/// through the lobe folded over it.
pub fn lift_for(
    mesh: &Mesh,
    bvh: &Bvh,
    at: Vec3,
    axis: Vec3,
    diameter_mm: Scalar,
    depth_mm: Scalar,
) -> Scalar {
    let Some(axis) = axis.try_normalize() else {
        return 0.0;
    };
    let radius = diameter_mm / 2.0;
    let (right, up) = frame(axis);

    (0..RIM_SAMPLES)
        .filter_map(|sample| {
            let angle = std::f32::consts::TAU * sample as Scalar / RIM_SAMPLES as Scalar;
            let (sin, cos) = angle.sin_cos();
            let rim = at + (right * cos + up * sin) * radius;
            // Started just under the mouth's plane, so a flat face the rim lies on is met.
            let ray = Ray::new(rim + axis * MOUTH_LIFT_MM, -axis);
            let hit = bvh.raycast(&ray)?;
            let leaves = mesh.triangle(hit.face)?.normal_unnormalized().dot(axis) < 0.0;
            let rise = hit.t - MOUTH_LIFT_MM;
            (leaves && rise <= depth_mm).then_some(rise)
        })
        .fold(0.0, Scalar::max)
}

/// A hole of `size` through the surface nearest `near`, drilled straight into it.
///
/// What the CLI and the trapped-volume report place a hole with, where the window has the
/// face under the cursor already. Returns `None` for a mesh with no faces.
pub fn hole_at(mesh: &Mesh, bvh: &Bvh, near: Vec3, size: HoleSize) -> Option<DrainHole> {
    let closest = bvh.closest(near)?;
    let axis = -mesh.triangle(closest.face)?.normal_unnormalized();
    let lift_mm = lift_for(
        mesh,
        bvh,
        closest.point,
        axis,
        size.diameter_mm,
        size.depth_mm,
    );
    Some(DrainHole::sized(closest.point, axis, size, lift_mm))
}

/// A channel under a run of points clicked on a surface, each with its outward normal.
///
/// The spine dives a radius clear under every point, so the tube runs through the model
/// instead of grazing the surface it was drawn on, and only its two ends climb back out to
/// open on it. `None` when fewer than two points have a normal to dive along.
pub fn channel_under(surface: &[(Vec3, Vec3)], diameter_mm: Scalar) -> Option<Channel> {
    let radius = diameter_mm / 2.0;
    if radius <= 0.0 || !radius.is_finite() {
        return None;
    }

    let dive = radius + MOUTH_LIFT_MM;
    let ends: Vec<(Vec3, Vec3)> = surface
        .iter()
        .filter_map(|(point, normal)| {
            let normal = normal.try_normalize()?;
            Some((*point + normal * MOUTH_LIFT_MM, *point - normal * dive))
        })
        .collect();
    if ends.len() < 2 {
        return None;
    }

    let mut points = Vec::with_capacity(ends.len() + 2);
    points.push(ends[0].0);
    points.extend(ends.iter().map(|(_, under)| *under));
    points.push(ends[ends.len() - 1].0);
    Some(Channel {
        points,
        diameter_mm,
        ..Channel::default()
    })
}

/// The cone one hole cuts, or `None` when it is not a hole at all.
pub(crate) fn body_of(hole: &DrainHole) -> Option<Mesh> {
    let axis = hole.axis.try_normalize()?;
    let radius = hole.diameter_mm / 2.0;
    let drillable =
        radius > 0.0 && hole.depth_mm > 0.0 && hole.taper > 0.0 && hole.taper.is_finite();
    if !drillable {
        return None;
    }

    let mouth = hole.at - axis * hole.lift_mm.max(MOUTH_LIFT_MM);
    let tip = hole.at + axis * hole.depth_mm;
    Some(tube(
        mouth,
        tip,
        radius,
        radius * hole.taper,
        Section::of(hole.shape, hole.turn_deg),
    ))
}

/// How a tube's ring is laid out about its axis: its sides, and the turn of the first
/// corner off the axis's [`frame`], radians.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Section {
    pub sides: usize,
    pub turn_rad: Scalar,
}

impl Section {
    pub(crate) fn of(shape: HoleShape, turn_deg: Scalar) -> Self {
        Self {
            sides: shape.sides(),
            turn_rad: turn_deg.to_radians(),
        }
    }

    /// The first of [`Self::corners`] about `axis` and the count of them, for a polygon.
    fn first_corner(self, shape: HoleShape, axis: Vec3) -> Option<(Vec3, usize)> {
        let sides = shape.corners()?;
        Some((self.corners(axis.try_normalize()?).next()?, sides))
    }

    /// Unit offsets from the axis to each corner, around `axis` the way [`frame`] turns.
    pub(crate) fn corners(self, axis: Vec3) -> impl Iterator<Item = Vec3> {
        let (right, up) = frame(axis);
        let (sin, cos) = self.turn_rad.sin_cos();
        around(axis, right * cos + up * sin, self.sides)
    }
}

/// `sides` unit offsets evenly round unit `axis`, the first at `first`, square to it.
pub(crate) fn around(axis: Vec3, first: Vec3, sides: usize) -> impl Iterator<Item = Vec3> {
    let across = axis.cross(first);
    (0..sides).map(move |side| {
        let (sin, cos) = (std::f32::consts::TAU * side as Scalar / sides as Scalar).sin_cos();
        first * cos + across * sin
    })
}

/// The turn about `axis`, in a model's own space, that puts a polygon's first corner
/// `turn_deg` off the plate's level once `linear` stands the model on the plate.
pub fn turn_in_model(axis: Vec3, turn_deg: Scalar, linear: Mat3) -> Scalar {
    let (Some(axis), Some(placed)) = (axis.try_normalize(), (linear * axis).try_normalize()) else {
        return turn_deg;
    };
    let (right, up) = frame(placed);
    let (sin, cos) = turn_deg.to_radians().sin_cos();
    let corner = linear.inverse() * (right * cos + up * sin);
    let (right, up) = frame(axis);
    corner.dot(up).atan2(corner.dot(right)).to_degrees()
}

/// A closed frustum of `section` from `from` to `to`, wound outward; `drill` turns it
/// round.
fn tube(from: Vec3, to: Vec3, radius_from: Scalar, radius_to: Scalar, section: Section) -> Mesh {
    let axis = (to - from).normalize_or_zero();
    let sides = section.sides;

    let mut vertices = Vec::with_capacity(2 * sides + 2);
    for offset in section.corners(axis) {
        vertices.push(from + offset * radius_from);
        vertices.push(to + offset * radius_to);
    }
    let (low_cap, high_cap) = (vertices.len() as u32, vertices.len() as u32 + 1);
    vertices.push(from);
    vertices.push(to);

    let mut faces = Vec::with_capacity(4 * sides);
    for side in 0..sides as u32 {
        let next = (side + 1) % sides as u32;
        let (low, high) = (2 * side, 2 * side + 1);
        let (low_next, high_next) = (2 * next, 2 * next + 1);
        faces.push([low, low_next, high_next]);
        faces.push([low, high_next, high]);
        faces.push([low_cap, low_next, low]);
        faces.push([high_cap, high, high_next]);
    }
    Mesh::new(vertices, faces)
}

/// Two unit vectors across `axis`, for laying a ring out around it. The first is level
/// wherever the axis is not upright, so a polygon's turn is measured from the level.
pub(crate) fn frame(axis: Vec3) -> (Vec3, Vec3) {
    let aside = if axis.z.abs() < 0.9 { Vec3::Z } else { Vec3::X };
    let right = axis.cross(aside).normalize_or_zero();
    (right, axis.cross(right))
}

pub(crate) fn append(whole: &mut Mesh, part: &Mesh) {
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

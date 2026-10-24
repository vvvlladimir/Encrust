use serde::{Deserialize, Serialize};

use crate::group::Profiles;
use crate::placed::Placed;
use core_geometry::{Mat3, Ray, Scalar, Vec2, Vec3, point_triangle};
use printer_profiles::SupportProfile;
use rayon::prelude::*;

use crate::clear::{Beam, MIN_BODY_MM, clear};
use crate::mesher::FOOT_BEVEL;
use crate::pad::{PadCut, pad_clear, pad_cut};

/// Plate coordinates put the plate surface at z = 0; see `docs/architecture.md`.
pub(crate) const PLATE_Z_MM: Scalar = 0.0;

/// How far below the contact a downward ray starts so it misses the face the contact sits on;
/// a micrometre is under any printable feature and above float noise at plate scale.
pub(crate) const SURFACE_EPSILON_MM: Scalar = 1e-3;

/// A column shorter than this is a bump, not a support. It is dropped rather than meshed
/// into a stub that turns itself inside out.
pub(crate) const MIN_PILLAR_HEIGHT_MM: Scalar = 0.2;

/// How near the surface a contact is read as sitting on it, millimetres of the model's own
/// space: a click lands on it to within float error.
const ON_SURFACE_MM: Scalar = 1e-3;

/// How far under a face a contact without a bite is moved up onto it, millimetres:
/// further than a layer of any resin printer, and nearer than a contact put in the air.
const ONTO_SURFACE_MM: Scalar = 0.5;

/// Ground anything carrying a support may cover per mm it descends: 45 degrees off vertical,
/// past which a strut sags as an overhang; a branch keeps its profile's angle (ADR 0249).
pub(crate) const MAX_LEAN_PER_DROP: Scalar = 1.0;

/// How far a face a support stands on may lean from horizontal, degrees. Past this the
/// foot has no surface under it: the column runs down the face instead of onto it.
pub(crate) const MAX_LANDING_SLOPE_DEG: Scalar = 45.0;

/// Where a support touches the model, in the model's own space, and how it is built.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SupportPoint {
    pub contact: Vec3,
    /// Which group's profile gives this support its shape; see [`crate::Profiles`].
    pub group: u16,
}

impl SupportPoint {
    /// A support of the default group, free to be merged into a trunk with its
    /// neighbours.
    pub fn new(contact: Vec3) -> Self {
        Self { contact, group: 0 }
    }

    /// The same support built to `group`'s profile.
    #[must_use]
    pub fn in_group(self, group: u16) -> Self {
        Self { group, ..self }
    }
}

/// Where a column dropped straight down from its contact comes to rest.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Landing {
    /// The resting point, in plate coordinates.
    pub base: Vec3,
    /// It landed on the model rather than on the plate, so it needs no foot.
    pub on_model: bool,
    /// How far the middle of the pad stands off `base` across the plate, mm; absent reads as under
    /// the trunk (ADR 0239).
    #[serde(default)]
    pub pad_offset: Vec2,
    /// The pad cut back where the model stands beside it, or `None` for a whole one (ADR 0328).
    #[serde(default)]
    pub pad_cut: Option<PadCut>,
    /// Landed on the trunk of a thicker support of another group at `base`; `on_model` is set with
    /// it and no foot is needed. Absent reads as landing on the model (ADR 0248).
    #[serde(default)]
    pub on_support: bool,
}

impl Landing {
    /// Height at which the pillar body ends: the top of the foot or the raft, or the surface
    /// it sank into.
    pub fn pillar_bottom_z(&self, profile: &SupportProfile) -> Scalar {
        if self.on_support {
            // On the axis of the trunk it joins, which is already inside that trunk.
            self.base.z
        } else if self.on_model {
            // Sunk into what it stands on, so the two cure as one solid.
            self.base.z - profile.landing_depth_mm()
        } else {
            self.base.z + profile.footing_height_mm()
        }
    }

    /// The middle of the pad on the plate.
    pub fn pad_centre(&self) -> Vec3 {
        self.base + self.pad_offset.extend(0.0)
    }

    /// Lowest point of the whole column.
    pub fn bottom_z(&self, profile: &SupportProfile) -> Scalar {
        if self.on_model {
            self.pillar_bottom_z(profile)
        } else {
            self.base.z
        }
    }
}

/// Height a trunk of `radius_mm` standing on `landing` is swept for clearance down to: the top
/// of the foot on the plate, where the trunk's rim may already touch the face on the model.
pub(crate) fn swept_bottom_z(
    placed: &Placed,
    landing: &Landing,
    radius_mm: Scalar,
    profile: &SupportProfile,
) -> Scalar {
    if !landing.on_model {
        return landing.pillar_bottom_z(profile);
    }
    // The face under it may be one stood on as a last resort, steeper than a column
    // chooses; a landing on another support has no face of the model at its base.
    let ray = Ray::new(landing.base + Vec3::Z * SURFACE_EPSILON_MM, -Vec3::Z);
    let chosen = MAX_LANDING_SLOPE_DEG.to_radians().tan();
    let lean = placed
        .raycast(&ray)
        .filter(|hit| hit.distance <= 2.0 * SURFACE_EPSILON_MM)
        .and_then(|hit| lean_of(hit.normal, LAST_RESORT_SLOPE_DEG))
        .map_or(chosen, |lean| lean.max(chosen));
    landing.base.z + (radius_mm + profile.clearance_mm) * lean + SURFACE_EPSILON_MM
}

/// One support resolved against the model: a tip square on its face and a vertical column
/// below it down to what it stands on, in plate coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Column {
    /// Index of the point this column grew from.
    pub point: usize,
    /// The group the point belongs to, carried through so that growing and meshing keep
    /// to one profile per support.
    pub group: u16,
    pub contact: Vec3,
    /// Where the body begins: one `[top]` out from the contact along the normal of the
    /// face it holds, so the tip meets that face square on (ADR 0125).
    pub neck: Vec3,
    /// Where a column straight down from the neck comes to rest, or `None` when one
    /// cannot: the tip then only stands if a branch carries it somewhere that can.
    pub landing: Option<Landing>,
}

impl Column {
    /// Height from the top of the foot to the neck, in millimetres, for a tip that has
    /// somewhere of its own to stand.
    pub fn pillar_height_mm(&self, profile: &SupportProfile) -> Option<Scalar> {
        Some(self.neck.z - self.landing?.pillar_bottom_z(profile))
    }
}

/// Resolves model-space points to plate-coordinate columns, in order, on every core (ADR 0346).
/// No landing: kept for [`crate::grow`] to carry; no head square on its face: dropped.
pub fn columns(points: &[SupportPoint], placed: &Placed, profiles: Profiles) -> Vec<Column> {
    let matrix = placed.transform.to_matrix();
    points
        .par_iter()
        .enumerate()
        .filter_map(|(point, support)| {
            let contact = matrix.transform_point3(support.contact);
            column(
                placed,
                point,
                support.group,
                contact,
                profiles.of(support.group),
            )
        })
        .collect()
}

/// The column of point `point` of `group`, touching the model at `contact` in plate
/// coordinates, or `None` when its head cannot leave its face square on.
pub(crate) fn column(
    placed: &Placed,
    point: usize,
    group: u16,
    contact: Vec3,
    profile: &SupportProfile,
) -> Option<Column> {
    let contact = onto_surface(placed, contact, profile);
    let neck = neck_of(placed, contact, profile)?;
    Some(Column {
        point,
        group,
        contact,
        neck,
        landing: landing(placed, neck, profile.pillar_radius_mm(), 1, profile),
    })
}

/// The contact a support put down at `contact` holds and the neck of its head square on
/// its face, whether or not that head is clear: what a hand is shown before it places one.
pub(crate) fn square_head(
    placed: &Placed,
    contact: Vec3,
    profile: &SupportProfile,
) -> (Vec3, Vec3) {
    let contact = onto_surface(placed, contact, profile);
    let (direction, _) = tip_direction(placed, contact);
    (contact, neck_along(contact, direction, profile))
}

/// `contact` moved down onto the underside under it when inside the model, else up onto the
/// face over it when the tip does not bite (ADR 0323, 0330).
fn onto_surface(placed: &Placed, contact: Vec3, profile: &SupportProfile) -> Vec3 {
    if on_surface(placed, contact) {
        return contact;
    }
    // Leaving the model downwards is the one way out through a face that looks down.
    let down = Ray::new(contact, -Vec3::Z);
    if let Some(hit) = placed.raycast(&down).filter(|hit| hit.normal.z < 0.0) {
        return hit.point;
    }
    if profile.tip.contact_depth_mm > 0.0 {
        return contact;
    }
    let ray = Ray::new(contact, Vec3::Z);
    placed
        .raycast(&ray)
        .filter(|hit| hit.distance <= ONTO_SURFACE_MM && hit.normal.z < 0.0)
        .map_or(contact, |hit| hit.point)
}

/// Whether `contact`, in plate coordinates, already sits on the model's surface.
fn on_surface(placed: &Placed, contact: Vec3) -> bool {
    let Some(inverse) = placed.transform.inverse_matrix() else {
        return false;
    };
    let local = inverse.transform_point3(contact);
    placed
        .bvh
        .closest(local)
        .is_some_and(|found| found.distance <= ON_SURFACE_MM)
}

/// Where a trunk carrying `tips` dropped from `from` (plate coordinates) lands, or `None` when
/// there is no room: nothing to stand on, model in the way, painted-out face (ADR 0242).
pub fn landing(
    placed: &Placed,
    from: Vec3,
    radius_mm: Scalar,
    tips: usize,
    profile: &SupportProfile,
) -> Option<Landing> {
    landing_within(
        placed,
        from,
        radius_mm,
        tips,
        profile,
        MAX_LANDING_SLOPE_DEG,
    )
}

/// How far a face may lean and still be stood on by a support with nowhere better to go,
/// degrees: short of a wall, which a trunk would only run down.
pub(crate) const LAST_RESORT_SLOPE_DEG: Scalar = 75.0;

/// [`landing`], onto a face of the model leaning no further than `steepest_deg`.
pub(crate) fn landing_within(
    placed: &Placed,
    from: Vec3,
    radius_mm: Scalar,
    tips: usize,
    profile: &SupportProfile,
    steepest_deg: Scalar,
) -> Option<Landing> {
    let contact = from;
    let ray = Ray::new(contact - Vec3::Z * SURFACE_EPSILON_MM, -Vec3::Z);
    let below = placed.raycast(&ray);

    // Where the body stops being asked about: at the top of the foot on the plate, and on
    // the model at the height its rim already meets the face it stands on.
    let (landing, bottom_z, base_mm) = match below {
        // A foot may not be put down on a blocked face; see `docs/decisions/0093`.
        Some(hit) if placed.blocks_at(hit.point) => return None,
        // The profile forbids standing on the part, so the support leans for the plate
        // instead; see `docs/decisions/0124`.
        Some(hit) if hit.point.z > PLATE_Z_MM && !profile.land_on_model => return None,
        Some(hit) if hit.point.z > PLATE_Z_MM => {
            // The whole beam meets the face it lands on, clearance and all, so the reach
            // that stops being asked about is measured on the beam's own width.
            let touch_mm = (radius_mm + profile.clearance_mm) * lean_of(hit.normal, steepest_deg)?;
            (
                Landing {
                    base: hit.point,
                    on_model: true,
                    pad_offset: Vec2::ZERO,
                    pad_cut: None,
                    on_support: false,
                },
                hit.point.z + touch_mm + SURFACE_EPSILON_MM,
                radius_mm,
            )
        }
        _ => {
            let base = Vec3::new(contact.x, contact.y, PLATE_Z_MM);
            let base_mm = profile
                .trunk_base_radius_mm(from.z - PLATE_Z_MM, tips)
                .max(radius_mm);
            let (pad_offset, pad_cut) = pad_offset(placed, base, base_mm, profile)?;
            let landing = Landing {
                base,
                on_model: false,
                pad_offset,
                pad_cut,
                on_support: false,
            };
            let bottom_z = landing.pillar_bottom_z(profile);
            (landing, bottom_z, base_mm)
        }
    };

    let height = contact.z - landing.pillar_bottom_z(profile);
    if height <= MIN_PILLAR_HEIGHT_MM {
        return None;
    }
    let body = Beam {
        from: contact,
        to: Vec3::new(contact.x, contact.y, bottom_z),
        from_radius_mm: radius_mm,
        to_radius_mm: base_mm,
    };
    stands_clear(placed, &body, profile).then_some(landing)
}

/// Where the body of a support at `contact` begins: one `[top]` out along the face normal so
/// only the tip meets the face (ADR 0125, 0346); `None` if a head along it would hit the model.
pub fn neck_of(placed: &Placed, contact: Vec3, profile: &SupportProfile) -> Option<Vec3> {
    let (direction, normal) = tip_direction(placed, contact);
    let neck = neck_along(contact, direction, profile);
    if normal.is_none() {
        return Some(neck);
    }
    if has_head(contact, neck) && head_clear(placed, contact, neck, profile) {
        return (!over_its_contact(contact, neck, profile)).then_some(neck);
    }
    (profile.small_pillar.enabled && no_room_below(placed, contact, profile)).then_some(contact)
}

/// Whether a head rises so nearly straight up that the trunk dropped from its neck would
/// come down on its own contact.
fn over_its_contact(contact: Vec3, neck: Vec3, profile: &SupportProfile) -> bool {
    neck.z > contact.z
        && (neck - contact).truncate().length()
            < profile.pillar_radius_mm() + profile.contact_radius_mm()
}

/// Whether the model under `contact` is nearer than a head is long, so a head straight
/// down would run through it.
fn no_room_below(placed: &Placed, contact: Vec3, profile: &SupportProfile) -> bool {
    let ray = Ray::new(contact - Vec3::Z * SURFACE_EPSILON_MM, -Vec3::Z);
    placed
        .raycast(&ray)
        .is_some_and(|hit| hit.distance < profile.top.length_mm && hit.normal.z > 0.0)
}

/// Whether a support at `contact` leaves it by a head to `neck`. One without is a single
/// strut from the contact itself, which is how [`crate::grow`] stands it.
pub(crate) fn has_head(contact: Vec3, neck: Vec3) -> bool {
    contact.distance(neck) >= MIN_BODY_MM
}

/// Where a head leaving `contact` along `direction` ends: one `[top]` out, shortened
/// rather than allowed to eat the room the body needs above the plate.
fn neck_along(contact: Vec3, direction: Vec3, profile: &SupportProfile) -> Vec3 {
    let length_mm = profile.top.length_mm;
    let drop_mm = -direction.z * length_mm;
    let room_mm =
        (contact.z - PLATE_Z_MM - profile.footing_height_mm() - MIN_PILLAR_HEIGHT_MM).max(0.0);
    let length_mm = if drop_mm > room_mm {
        length_mm * room_mm / drop_mm
    } else {
        length_mm
    };
    contact + direction * length_mm
}

/// Whether the head from `contact` to `neck`, widening from the contact to the pillar,
/// meets nothing once it is a hair off the face it leaves square on.
fn head_clear(placed: &Placed, contact: Vec3, neck: Vec3, profile: &SupportProfile) -> bool {
    let head = Beam {
        from: contact,
        to: neck,
        from_radius_mm: profile.contact_radius_mm(),
        to_radius_mm: profile.pillar_radius_mm(),
    };
    clear(placed, &head.trimmed(10.0 * SURFACE_EPSILON_MM))
}

/// The outward normal of the face a tip at `contact` sits on and leaves, if it sits on
/// one; see [`tip_direction`].
pub(crate) fn face_normal(placed: &Placed, contact: Vec3) -> Option<Vec3> {
    tip_direction(placed, contact).1
}

/// Which way a tip leaves the surface at `contact`: the outward normal of a face looking down
/// or sat on, else straight down; on an edge the most downward-looking face wins.
fn tip_direction(placed: &Placed, contact: Vec3) -> (Vec3, Option<Vec3>) {
    let Some(inverse) = placed.transform.inverse_matrix() else {
        return (-Vec3::Z, None);
    };
    let local = inverse.transform_point3(contact);
    let Some(found) = placed.bvh.closest(local) else {
        return (-Vec3::Z, None);
    };
    // A normal does not survive a non-uniform scale under the model matrix, but it does
    // under the inverse transpose of that matrix's upper 3x3.
    let turn = Mat3::from_mat4(inverse).transpose();
    let normal_of = |face: usize| {
        placed
            .model
            .triangle(face)
            .map(|triangle| (turn * triangle.normal_unnormalized()).normalize_or_zero())
    };
    let Some(mut normal) = normal_of(found.face) else {
        return (-Vec3::Z, None);
    };

    let on_surface = found.distance <= ON_SURFACE_MM;
    if on_surface {
        let mut touching = Vec::new();
        placed.bvh.faces_within(local, ON_SURFACE_MM, &mut touching);
        // The hierarchy's leaves come back whole, so each face is measured again.
        normal = touching
            .into_iter()
            .filter_map(|face| Some((placed.model.triangle(face as usize)?, face)))
            .filter(|(triangle, _)| {
                point_triangle(local, triangle).distance(local) <= ON_SURFACE_MM
            })
            .filter_map(|(_, face)| normal_of(face as usize))
            .fold(
                normal,
                |lowest, other| {
                    if other.z < lowest.z { other } else { lowest }
                },
            );
    }
    if normal.z < 0.0 || on_surface {
        (normal, Some(normal))
    } else {
        (-Vec3::Z, None)
    }
}

/// How many directions and how many reaches a pad is tried at round its trunk before the
/// support has to lean for room instead.
const PAD_DIRECTIONS: u32 = 8;
const PAD_REACHES: u32 = 3;

/// Where the pad of a trunk of `radius_mm` at `base` goes and how it is cut: whole, moved aside
/// or cut back where the model stands, `None` if none fits (ADR 0239, 0328).
fn pad_offset(
    placed: &Placed,
    base: Vec3,
    radius_mm: Scalar,
    profile: &SupportProfile,
) -> Option<(Vec2, Option<PadCut>)> {
    if foot_clear(placed, base, profile) {
        return Some((Vec2::ZERO, None));
    }
    let shape = profile.bottom.shape;
    let inside_mm = profile.flare_lower_radius_mm().max(radius_mm);
    let cut = || {
        shape
            .is_round()
            .then(|| pad_cut(placed, base, inside_mm, profile))
            .flatten()
            .map(|cut| (Vec2::ZERO, Some(cut)))
    };
    let reach_mm = if shape.is_round() && !shape.tapers() {
        // Inside the rim the pad meets the plate on, which the bevel pulls in.
        profile.base_radius_mm() - profile.bottom.platform_thickness_mm * FOOT_BEVEL - inside_mm
    } else {
        0.0
    };
    if reach_mm <= SURFACE_EPSILON_MM {
        return cut();
    }

    (1..=PAD_REACHES)
        .flat_map(|step| (0..PAD_DIRECTIONS).map(move |turn| (step, turn)))
        .map(|(step, turn)| {
            let angle = std::f32::consts::TAU * turn as Scalar / PAD_DIRECTIONS as Scalar;
            Vec2::new(angle.cos(), angle.sin())
                * (reach_mm * step as Scalar / PAD_REACHES as Scalar)
        })
        .find(|offset| foot_clear(placed, base + offset.extend(0.0), profile))
        .map(|offset| (offset, None))
        .or_else(cut)
}

/// Whether the foot on the plate has the model out of its way: a pad is wider than anything
/// above it and can hit a part beside it that the column's rays pass by (ADR 0328).
pub(crate) fn foot_clear(placed: &Placed, base: Vec3, profile: &SupportProfile) -> bool {
    pad_clear(placed, base, profile)
}

/// How much ground the face `normal` belongs to covers per millimetre it rises, or `None`
/// when it is too steep to stand a support on at all.
fn lean_of(normal: Vec3, steepest_deg: Scalar) -> Option<Scalar> {
    let up = normal.z;
    (up >= steepest_deg.to_radians().cos()).then(|| (1.0 - up * up).max(0.0).sqrt() / up)
}

/// Whether `body`, a trunk dropped from its joint, has the model out of its way; the sweep
/// starts below the top so a head off a steep face is not refused, that stretch bare (ADR 0328).
pub(crate) fn stands_clear(placed: &Placed, body: &Beam, profile: &SupportProfile) -> bool {
    let height_mm = body.from.z - body.to.z;
    if height_mm < MIN_BODY_MM {
        return true;
    }
    let under_the_head = body.from_radius_mm + profile.clearance_mm;
    let top = Beam {
        to: body.from - Vec3::Z * under_the_head.min(height_mm),
        to_radius_mm: body.from_radius_mm,
        ..*body
    };
    clear(placed, &top.trimmed(10.0 * SURFACE_EPSILON_MM))
        && clear(
            placed,
            &body
                .with_clearance(profile.clearance_mm)
                .trimmed(under_the_head),
        )
}

#[cfg(test)]
mod tests;

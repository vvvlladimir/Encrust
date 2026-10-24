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

/// How far below the contact a downward ray starts, so that it does not immediately hit
/// the face the contact sits on. A micrometre is under any printable feature and well
/// above the float noise at plate scale.
pub(crate) const SURFACE_EPSILON_MM: Scalar = 1e-3;

/// A column shorter than this is a bump, not a support. It is dropped rather than meshed
/// into a stub that turns itself inside out.
pub(crate) const MIN_PILLAR_HEIGHT_MM: Scalar = 0.2;

/// A transform this close to singular has no surface left to read a normal off.
const SINGULAR: Scalar = 1e-12;

/// How near the surface a contact is read as sitting on it, millimetres of the model's own
/// space: a click lands on it to within float error.
const ON_SURFACE_MM: Scalar = 1e-3;

/// How far under a face a contact without a bite is moved up onto it, millimetres:
/// further than a layer of any resin printer, and nearer than a contact put in the air.
const ONTO_SURFACE_MM: Scalar = 0.5;

/// How much ground anything that carries a support may cover per millimetre it descends,
/// branches and heads apart: 45 degrees off vertical, past which a strut is printed as an
/// overhang of its own and sags. A branch keeps its profile's angle. See
/// `docs/decisions/0249`.
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
    /// How far the middle of the pad stands off `base` across the plate, millimetres: a
    /// pad the model is in the way of is moved aside whole rather than narrowed. A file
    /// without it reads as a pad under its trunk. See `docs/decisions/0239`.
    #[serde(default)]
    pub pad_offset: Vec2,
    /// The pad cut back where the model stands beside it, or `None` for a whole one. A
    /// pad is cut before it is moved aside; a file without it reads as a whole pad. See
    /// `docs/decisions/0328`.
    #[serde(default)]
    pub pad_cut: Option<PadCut>,
    /// What it landed on is the trunk of a thicker support of another group, which it
    /// runs into at `base`; `on_model` is set with it, since it needs no foot either. A
    /// file without it reads as a landing on the model itself. See `docs/decisions/0248`.
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

/// Height a trunk of `radius_mm` standing on `landing` is swept for clearance down to.
///
/// On the plate that is the top of the foot. On the model it is where the trunk's rim,
/// clearance and all, may already touch the face it stands on: below that it is sinking
/// into what it stands on, which is what it is meant to do.
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

/// One support resolved against the model: a tip square on the face it holds, and a
/// vertical column from the end of that tip down to whatever it stands on, in plate
/// coordinates.
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

/// Resolves every point against the placed model.
///
/// A point whose own column has nowhere to stand is kept with no landing rather than
/// thrown away: [`crate::grow`] may still carry its tip to a trunk that can stand, and
/// what cannot be carried is dropped there. A point with no head square on its face is
/// left out here (ADR 0346).
///
/// Points arrive in the model's own space and the columns come back in plate coordinates,
/// so a column keeps the profile's thickness however the model is scaled.
///
/// One point is one ray and an automatic run places thousands of them, so they are
/// resolved on every core; the order they come back in is still the order they went in.
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

/// `contact` moved down onto the underside under it when it is inside the model, else up
/// onto the face over it when the profile's tip does not bite.
///
/// Automatic placement puts a contact a layer under the surface it holds, and a tip that
/// does not sink in from there never touches that surface; see `docs/decisions/0323`. A
/// support steadying a part is put inside its outline, which a tip built from there would
/// cure through; see `docs/decisions/0330`.
fn onto_surface(placed: &Placed, contact: Vec3, profile: &SupportProfile) -> Vec3 {
    if on_surface(placed, contact) {
        return contact;
    }
    // Leaving the model downwards is the one way out through a face that looks down.
    let down = Ray::new(contact, -Vec3::Z);
    if let Some(hit) = placed.raycast(&down).filter(|hit| hit.normal.z < 0.0) {
        return hit.point;
    }
    if profile.contact_depth_mm() > 0.0 {
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
    let matrix = placed.transform.to_matrix();
    if matrix.determinant().abs() < SINGULAR {
        return false;
    }
    let local = matrix.inverse().transform_point3(contact);
    placed
        .bvh
        .closest(local)
        .is_some_and(|found| found.distance <= ON_SURFACE_MM)
}

/// Where a trunk carrying `tips` dropped from `from` lands, or `None` when there is no
/// room for one: nothing to stand on, the model itself in the way of the body, a face the
/// user has painted supports out of, or a profile that refuses to stand on the part.
///
/// `from` is where the body begins — the neck of a tip, or a joint — in plate
/// coordinates, the same space `placed` puts the model in. The trunk leaves it `radius_mm`
/// wide and, on the plate, widens down to the base its height and load ask for; on the
/// model it keeps that width down to a tip. See `docs/decisions/0242`.
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

/// Where the body of a support placed at `contact` begins: one `[top]` out along the
/// normal of the face it holds, so that only the tip meets that face and leaves a mark of
/// the contact's width, whatever the branch under it does (ADR 0125, 0346).
///
/// `None` when a head along that normal would meet the model: such a support is not
/// placed, rather than turned to touch its face with more than its tip. A contact with the
/// model nearer under it than a head is long is its own neck where the profile has small
/// pillars, which run face to face (ADR 0331). The head is shortened rather than allowed
/// to eat the room the body needs above the plate.
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
        .is_some_and(|hit| hit.distance < profile.top_length_mm() && hit.normal.z > 0.0)
}

/// Whether a support at `contact` leaves it by a head to `neck`. One without is a single
/// strut from the contact itself, which is how [`crate::grow`] stands it.
pub(crate) fn has_head(contact: Vec3, neck: Vec3) -> bool {
    contact.distance(neck) >= MIN_BODY_MM
}

/// Where a head leaving `contact` along `direction` ends: one `[top]` out, shortened
/// rather than allowed to eat the room the body needs above the plate.
fn neck_along(contact: Vec3, direction: Vec3, profile: &SupportProfile) -> Vec3 {
    let length_mm = profile.top_length_mm();
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

/// Which way a tip leaves the surface at `contact`: the outward normal of a face that looks
/// down or that it sits on, whichever way that looks, so the tip meets its face square on;
/// straight down from a contact off the surface. The normal comes with it, where it leaves
/// a face.
///
/// A contact on an edge or a corner sits on every face meeting there, and leaves the one
/// that looks down most: which of them a search finds first is an accident of the mesh.
fn tip_direction(placed: &Placed, contact: Vec3) -> (Vec3, Option<Vec3>) {
    let matrix = placed.transform.to_matrix();
    if matrix.determinant().abs() < SINGULAR {
        return (-Vec3::Z, None);
    }
    let inverse = matrix.inverse();
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

/// Where the pad of a trunk of `radius_mm` standing at `base` goes, and how it is cut:
/// whole under the trunk, whole as little aside as clears the model, or cut back on the
/// side the model stands, and `None` when none of those does.
///
/// A pad is never narrowed all round. It is moved only as far as still leaves the flare
/// inside its top, which only a round pad that does not taper has room for, and cut only
/// where the model is; see `docs/decisions/0239` and `docs/decisions/0328`.
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
        profile.base_radius_mm() - profile.base_height_mm() * FOOT_BEVEL - inside_mm
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

/// Whether the foot on the plate has the model out of its way.
///
/// A pad is wider than anything above it, so a column that came down in clear air can
/// still drive its foot into a part that stands on the plate beside it, or round a toe
/// standing inside it, which rays down its rim and axis pass by (ADR 0328).
pub(crate) fn foot_clear(placed: &Placed, base: Vec3, profile: &SupportProfile) -> bool {
    pad_clear(placed, base, profile)
}

/// How much ground the face `normal` belongs to covers per millimetre it rises, or `None`
/// when it is too steep to stand a support on at all.
fn lean_of(normal: Vec3, steepest_deg: Scalar) -> Option<Scalar> {
    let up = normal.z;
    (up >= steepest_deg.to_radians().cos()).then(|| (1.0 - up * up).max(0.0).sqrt() / up)
}

/// Whether `body`, a trunk dropped from its joint, has the model out of its way.
///
/// The sweep begins the body's own width and clearance below its top. The top is where
/// a head leaves the face it holds, and a head off a steep face ends that near the
/// face: asking for clear air all round it there refuses the very surface it stands
/// under, which is how a contact under a rounded underside was left with no column. That
/// stretch is still kept out of the model itself, bare (ADR 0328).
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
mod tests {
    use super::*;
    use crate::tests::{box_mesh, columns_on, landing_on, profile, ramp, under_a_gap_over_a_bar};
    use core_geometry::{Bvh, Mesh, Transform};

    /// A wall filling 0..1 on x, standing on the plate.
    fn wall() -> Mesh {
        box_mesh(Vec3::new(0.0, -5.0, 0.0), Vec3::new(1.0, 5.0, 20.0))
    }

    /// A cube spanning 0..10 on every axis, standing on the plate.
    fn cube_on_plate() -> Mesh {
        box_mesh(Vec3::ZERO, Vec3::splat(10.0))
    }

    #[test]
    fn a_column_over_nothing_reaches_the_plate() {
        let landing = landing_on(
            &Mesh::default(),
            Transform::default(),
            Vec3::new(3.0, 4.0, 20.0),
            &profile(),
        )
        .expect("20 mm of clearance is enough for a column");

        assert!(!landing.on_model);
        assert!(landing.base.abs_diff_eq(Vec3::new(3.0, 4.0, 0.0), 1e-6));
    }

    #[test]
    fn a_column_over_a_blocked_face_has_nowhere_to_land() {
        let model = cube_on_plate();
        let bvh = Bvh::build(&model);
        let mut region = core_geometry::Region::default();
        for face in 0..model.faces.len() {
            region.set(face, true);
        }
        let blocked =
            crate::Blocked::new(&model, &region, Transform::default()).expect("a painted cube");
        let placed = Placed::new(&model, &bvh, Transform::default()).blocking(Some(&blocked));

        assert!(
            landing(
                &placed,
                Vec3::new(5.0, 5.0, 20.0),
                profile().pillar_radius_mm(),
                1,
                &profile()
            )
            .is_none(),
            "the only surface under the contact is painted out of bounds"
        );
    }

    #[test]
    fn a_column_over_the_model_has_nowhere_to_land_when_the_profile_refuses_the_part() {
        let mut profile = profile();
        profile.land_on_model = false;
        assert!(
            landing_on(
                &cube_on_plate(),
                Transform::default(),
                Vec3::new(5.0, 5.0, 20.0),
                &profile,
            )
            .is_none(),
            "the only surface under the contact is the part itself"
        );
    }

    #[test]
    fn a_column_over_the_model_lands_on_it() {
        let landing = landing_on(
            &cube_on_plate(),
            Transform::default(),
            Vec3::new(5.0, 5.0, 20.0),
            &profile(),
        )
        .expect("10 mm above the cube is enough for a column");

        assert!(landing.on_model);
        assert!(
            (landing.base.z - 10.0).abs() < 1e-4,
            "the cube's top face is at z = 10, got {}",
            landing.base.z
        );
    }

    #[test]
    fn a_column_landing_on_the_model_sinks_into_it() {
        let profile = profile();
        let landing = landing_on(
            &cube_on_plate(),
            Transform::default(),
            Vec3::new(5.0, 5.0, 20.0),
            &profile,
        )
        .expect("there is room");

        let expected = 10.0 - profile.contact_depth_mm();
        assert!(
            (landing.pillar_bottom_z(&profile) - expected).abs() < 1e-4,
            "the pillar must end {} mm inside the surface",
            profile.contact_depth_mm()
        );
        assert!((landing.bottom_z(&profile) - expected).abs() < 1e-4);
    }

    /// A slab spanning 10..12 in z, hanging in the air, and the column of a contact
    /// `under_mm` below its underside built to `profile`.
    fn column_under_a_slab(under_mm: Scalar, profile: &SupportProfile) -> Column {
        let model = box_mesh(Vec3::new(-5.0, -5.0, 10.0), Vec3::new(5.0, 5.0, 12.0));
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        column(&placed, 0, 0, Vec3::new(0.0, 0.0, 10.0 - under_mm), profile)
            .expect("straight down from the underside of a slab is open air")
    }

    #[test]
    fn a_contact_without_a_bite_a_layer_under_the_surface_is_put_on_it() {
        let mut flush = profile();
        flush.tip.contact_depth_mm = 0.0;
        let column = column_under_a_slab(0.05, &flush);
        assert!(
            (column.contact.z - 10.0).abs() < 1e-4,
            "the slab's underside is at z = 10, got {}",
            column.contact.z
        );
    }

    #[test]
    fn a_contact_that_bites_stays_where_it_was_put() {
        let column = column_under_a_slab(0.05, &profile());
        assert!((column.contact.z - 9.95).abs() < 1e-4);
    }

    #[test]
    fn a_contact_without_a_bite_well_under_the_surface_stays_where_it_was_put() {
        let mut flush = profile();
        flush.tip.contact_depth_mm = 0.0;
        let column = column_under_a_slab(3.0, &flush);
        assert!(
            (column.contact.z - 7.0).abs() < 1e-4,
            "3 mm of air is a contact off the surface, not one a layer short of it"
        );
    }

    #[test]
    fn a_contact_inside_the_part_is_put_on_the_underside_under_it() {
        let mut flush = profile();
        flush.tip.contact_depth_mm = 0.0;
        for profile in [&flush, &profile()] {
            let column = column_under_a_slab(-1.0, profile);
            assert!(
                (column.contact.z - 10.0).abs() < 1e-4,
                "1 mm inside a slab whose underside is at z = 10, got {}",
                column.contact.z
            );
        }
    }

    #[test]
    fn a_column_on_the_plate_stands_on_its_foot() {
        let profile = profile();
        let landing = landing_on(
            &Mesh::default(),
            Transform::default(),
            Vec3::new(1.0, 1.0, 20.0),
            &profile,
        )
        .expect("there is room");

        assert!((landing.pillar_bottom_z(&profile) - profile.base_height_mm()).abs() < 1e-6);
        assert!(
            landing.bottom_z(&profile).abs() < 1e-6,
            "the foot is on z = 0"
        );
    }

    #[test]
    fn the_light_preset_stands_a_support_under_a_part_at_its_own_lift() {
        let profile = SupportProfile::shipped("light").expect("the light preset ships");
        let contact = Vec3::new(1.0, 1.0, profile.z_lift_mm);
        assert!(
            landing_on(&Mesh::default(), Transform::default(), contact, &profile).is_some(),
            "the pad, the flare and the top segment have to stand inside the {} mm this \
             profile lifts a part by",
            profile.z_lift_mm
        );
    }

    #[test]
    fn a_contact_with_no_room_under_it_has_no_landing() {
        let profile = profile();
        // The foot alone is taller than this contact stands.
        let contact = Vec3::new(1.0, 1.0, profile.base_height_mm());
        assert!(landing_on(&Mesh::default(), Transform::default(), contact, &profile).is_none());
    }

    #[test]
    fn the_ray_does_not_land_on_the_face_it_started_from() {
        // A box hanging 10 mm over the plate, with the contact on its underside: exactly
        // where a support belongs. A ray from the contact itself would hit that same face
        // at zero distance and report a column of no height standing on the model.
        let hanging = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 20.0));
        let landing = landing_on(
            &hanging,
            Transform::default(),
            Vec3::new(5.0, 5.0, 10.0),
            &profile(),
        )
        .expect("10 mm of clear air under the contact is room for a column");

        assert!(!landing.on_model, "there is nothing under the overhang");
        assert!((landing.base.z - 0.0).abs() < 1e-6);
    }

    #[test]
    fn a_column_that_would_run_down_the_side_of_the_model_is_not_built() {
        // A 1.2 mm body with half a millimetre of air round it needs 1.1 mm between its
        // axis and the wall's face at x = 1. This one has half of that.
        let contact = Vec3::new(1.5, 0.0, 20.0);
        assert!(landing_on(&wall(), Transform::default(), contact, &profile()).is_none());
    }

    #[test]
    fn a_column_with_room_beside_the_model_still_stands() {
        let contact = Vec3::new(3.0, 0.0, 20.0);
        let landing = landing_on(&wall(), Transform::default(), contact, &profile())
            .expect("2 mm of air beside the wall is room for a 1.2 mm body");
        assert!(!landing.on_model, "it went past the wall to the plate");
    }

    #[test]
    fn a_column_will_not_stand_on_a_face_that_leans_too_far() {
        // A face rising 10 mm over 4 leans 68 degrees from horizontal: a foot on it has
        // nothing under it, and the body would run down the face rather than onto it.
        let steep = ramp(4.0, 10.0);
        let contact = Vec3::new(2.0, 0.0, 20.0);
        assert!(landing_on(&steep, Transform::default(), contact, &profile()).is_none());
    }

    #[test]
    fn a_column_stands_on_a_face_that_leans_within_the_limit() {
        // The same ramp the other way up: 4 mm over 10 is 22 degrees.
        let gentle = ramp(10.0, 4.0);
        let contact = Vec3::new(5.0, 0.0, 20.0);
        let landing = landing_on(&gentle, Transform::default(), contact, &profile())
            .expect("a 22 degree face is something to stand on");
        assert!(landing.on_model);
        assert!(
            (landing.base.z - 2.0).abs() < 1e-3,
            "got {}",
            landing.base.z
        );
    }

    #[test]
    fn a_pad_the_model_is_in_the_way_of_is_moved_aside_whole() {
        // A step 2 mm tall whose face is at x = 1. The body comes down 3 mm clear of it,
        // but the pad is 8 mm across and keeps 0.5 mm of air, so its middle has to stand
        // 4.5 mm off that face: 1.5 mm further out than the trunk does.
        let kerb = box_mesh(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(1.0, 10.0, 2.0));
        let profile = profile();
        let landing = landing_on(
            &kerb,
            Transform::default(),
            Vec3::new(4.0, 0.0, 20.0),
            &profile,
        )
        .expect("the pad has 2.3 mm to move in before the flare leaves its sole");
        assert!(
            landing.pad_centre().x >= 5.5 - 1e-3,
            "the pad stands clear of the kerb, at {}",
            landing.pad_centre()
        );
        assert!(
            landing.pad_offset.length()
                <= profile.base_radius_mm() - profile.flare_lower_radius_mm() + 1e-4,
            "and the flare is still inside it, {} mm off its middle",
            landing.pad_offset.length()
        );

        let clear = landing_on(
            &kerb,
            Transform::default(),
            Vec3::new(6.0, 0.0, 20.0),
            &profile,
        )
        .expect("6 mm out the pad clears the kerb where it is");
        assert_eq!(clear.pad_offset, Vec2::ZERO);
    }

    #[test]
    fn a_pad_with_nowhere_to_move_is_cut_back_on_the_kerbs_side() {
        // The same step with the trunk 1.9 mm off it: the pad would have to move 2.6 mm
        // and has 2.3, but cut back to 1.4 mm on that side it still holds the 1.1 mm flare.
        let kerb = box_mesh(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(1.0, 10.0, 2.0));
        let landing = landing_on(
            &kerb,
            Transform::default(),
            Vec3::new(2.9, 0.0, 20.0),
            &profile(),
        )
        .expect("a cut pad has room");
        let cut = landing.pad_cut.expect("the pad is cut, not moved");
        assert_eq!(landing.pad_offset, Vec2::ZERO);
        assert!(
            (cut.reach_at(std::f32::consts::PI) - 1.4).abs() < 1e-3,
            "it stops its clearance short of the kerb, got {}",
            cut.reach_at(std::f32::consts::PI)
        );
    }

    #[test]
    fn a_toe_standing_inside_a_pad_is_in_its_way() {
        // A toe 1 mm across standing 2.5 mm out from the pad's middle, between its axis and
        // its rim, where no ray down the pad's rim or axis meets it.
        let toe = box_mesh(Vec3::new(2.0, 1.0, 0.0), Vec3::new(3.0, 2.0, 3.0));
        let bvh = Bvh::build(&toe);
        let placed = Placed::new(&toe, &bvh, Transform::default());
        assert!(!foot_clear(&placed, Vec3::ZERO, &profile()));
        assert!(
            foot_clear(&placed, Vec3::new(-10.0, 0.0, 0.0), &profile()),
            "a pad well away from it is clear"
        );
    }

    #[test]
    fn a_pad_with_nowhere_to_go_is_no_landing() {
        // The same step with the trunk 1.3 mm off it: the pad would have to move 3.2 mm,
        // and has 2.3, and cut back it would bare the flare.
        let kerb = box_mesh(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(1.0, 10.0, 2.0));
        assert!(
            landing_on(
                &kerb,
                Transform::default(),
                Vec3::new(2.3, 0.0, 20.0),
                &profile()
            )
            .is_none()
        );
    }

    #[test]
    fn points_are_resolved_through_the_models_placement() {
        let profile = profile();
        let transform = Transform::from_translation(Vec3::new(50.0, 50.0, 0.0));
        let points = [SupportPoint::new(Vec3::new(5.0, 5.0, 20.0))];

        let columns = columns_on(
            &points,
            &cube_on_plate(),
            transform,
            Profiles::single(&profile),
        );
        assert_eq!(columns.len(), 1);
        assert!(
            columns[0]
                .contact
                .abs_diff_eq(Vec3::new(55.0, 55.0, 20.0), 1e-5),
            "the contact follows the model onto the plate"
        );
        assert!(
            columns[0]
                .landing
                .expect("the cube is under the contact")
                .on_model,
            "the cube moved under it too"
        );
    }

    #[test]
    fn a_point_with_nowhere_to_stand_keeps_its_place_and_has_no_landing() {
        // Kept rather than thrown away: `grow` is where a tip that cannot stand on its
        // own is carried to a trunk that can, or finally dropped.
        let points = [
            SupportPoint::new(Vec3::new(1.0, 1.0, 0.05)),
            SupportPoint::new(Vec3::new(2.0, 2.0, 20.0)),
        ];
        let columns = columns_on(
            &points,
            &Mesh::default(),
            Transform::default(),
            Profiles::single(&profile()),
        );

        assert_eq!(columns.len(), 2);
        assert!(columns[0].landing.is_none(), "it stands under its own foot");
        assert!(columns[1].landing.is_some());
        assert_eq!(columns[1].point, 1, "every column knows its point");
    }

    #[test]
    fn a_tip_placed_on_a_face_looking_up_leaves_it_along_its_normal() {
        let model = ramp(4.0, 10.0);
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());

        // The middle of the sloping face, which looks up and out towards -X.
        let contact = Vec3::new(2.0, 0.0, 5.0);
        let neck = neck_of(&placed, contact, &profile()).expect("open air above the slope");
        let normal = Vec3::new(-10.0, 0.0, 4.0).normalize();
        assert!(
            (neck - contact).normalize().abs_diff_eq(normal, 1e-4),
            "only a head square on its face touches it with no more than its tip, got {neck}"
        );
    }

    #[test]
    fn a_support_placed_on_a_side_wall_still_stands() {
        // A box raised 10 mm off the plate, clicked on its side at x = 10.
        let raised = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 20.0));
        let trees = crate::tests::trees_on(
            &[SupportPoint::new(Vec3::new(10.0, 5.0, 15.0))],
            &raised,
            Transform::default(),
            Profiles::single(&profile()),
        );
        assert_eq!(
            trees.len(),
            1,
            "the click on the wall is held up from the plate"
        );
        assert!(
            trees[0].landing().base.x > 10.0,
            "it stands out beside the wall, not under it"
        );
    }

    #[test]
    fn a_tip_on_a_corner_leaves_the_face_that_looks_down_whichever_face_is_found_first() {
        use core_geometry::Quat;

        // A box tipped five degrees about X and then about Y, so one corner of its
        // underside hangs lowest and both walls meeting there look a little down too.
        let model = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
        let transform = Transform {
            rotation: Quat::from_rotation_y(5.0_f32.to_radians())
                * Quat::from_rotation_x(-5.0_f32.to_radians()),
            translation: Vec3::new(0.0, 0.0, 20.0),
            ..Transform::default()
        };
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, transform);
        let matrix = transform.to_matrix();

        let underside = matrix.transform_vector3(-Vec3::Z);
        for corner in [
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(10.0, 10.0, 0.0),
            Vec3::new(0.0, 10.0, 0.0),
        ] {
            let contact = matrix.transform_point3(corner);
            let neck = neck_of(&placed, contact, &profile()).expect("air under the box");
            let along = (neck - contact).normalize();
            assert!(
                along.dot(underside) > 0.999,
                "of the three faces meeting at {corner} the underside looks down most, \
                 and the tip left along {along}"
            );
        }
    }

    #[test]
    fn a_tip_leaves_a_sloping_face_along_its_normal() {
        // The face rises 4 over a run of 10, so once turned over its normal lies at
        // (-4, 0, -10): 22 degrees off straight down.
        let along = tip_under_a_turned_ramp(10.0, 4.0);
        let expected = Vec3::new(-4.0, 0.0, -10.0).normalize();
        assert!(
            along.normalize().abs_diff_eq(expected, 1e-4),
            "the tip leaves the face square on, got {}",
            along.normalize()
        );
        assert!(
            (along.length() - profile().top_length_mm()).abs() < 1e-4,
            "the tip is one top segment long"
        );
    }

    #[test]
    fn a_tip_under_a_face_near_vertical_meets_it_square_on() {
        // Rising 10 over a run of 4: the normal is 68 degrees off straight down, past
        // the 60 degrees a head was once held to.
        let along = tip_under_a_turned_ramp(4.0, 10.0).normalize();
        let expected = Vec3::new(-10.0, 0.0, -4.0).normalize();
        assert!(
            along.abs_diff_eq(expected, 1e-4),
            "the head follows the normal however steep the face, got {along}"
        );
    }

    /// From the contact to the neck of a tip in the middle of the sloping face of
    /// `ramp(run, rise)` turned over, so that the face is an overhang.
    fn tip_under_a_turned_ramp(run: Scalar, rise: Scalar) -> Vec3 {
        use core_geometry::Quat;
        use std::f32::consts::PI;

        let model = ramp(run, rise);
        let transform = Transform {
            rotation: Quat::from_rotation_x(PI),
            translation: Vec3::new(0.0, 0.0, 30.0),
            ..Transform::default()
        };
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, transform);
        let contact = transform
            .to_matrix()
            .transform_point3(Vec3::new(run / 2.0, 0.0, rise / 2.0));
        neck_of(&placed, contact, &profile()).expect("open air under the turned ramp") - contact
    }

    #[test]
    fn a_trunk_stands_on_a_steep_face_only_when_asked_as_a_last_resort() {
        // A ramp rising 10 over a run of 4: 68 degrees, past what a column chooses to
        // stand on and short of a wall.
        let model = ramp(4.0, 10.0);
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let from = Vec3::new(2.0, 0.0, 20.0);
        let radius_mm = profile().pillar_radius_mm();

        assert!(landing(&placed, from, radius_mm, 1, &profile()).is_none());
        let steep = landing_within(
            &placed,
            from,
            radius_mm,
            1,
            &profile(),
            LAST_RESORT_SLOPE_DEG,
        )
        .expect("there is a face under it, and nowhere better was the caller's to say");
        assert!(steep.on_model);
    }

    #[test]
    fn a_head_that_would_run_into_a_wall_across_from_it_is_not_placed() {
        // A contact on the side of one wall, with another standing a millimetre across.
        let mut model = box_mesh(Vec3::new(0.0, -5.0, 0.0), Vec3::new(1.0, 5.0, 20.0));
        let across = box_mesh(Vec3::new(2.0, -5.0, 0.0), Vec3::new(3.0, 5.0, 20.0));
        let offset = model.vertices.len() as u32;
        model.vertices.extend_from_slice(&across.vertices);
        model
            .faces
            .extend(across.faces.iter().map(|face| face.map(|at| at + offset)));
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());

        let neck = neck_of(&placed, Vec3::new(1.0, 0.0, 10.0), &profile());
        assert_eq!(
            neck, None,
            "turned away, the head would meet its wall slantwise"
        );
    }

    #[test]
    fn a_head_beside_a_lip_is_not_placed_rather_than_turned() {
        // A slab's underside at z = 10, and a lip 1.5 mm deep hanging from it at x = 10:
        // straight down from 0.3 mm short of the lip, the widening head runs into it.
        let mut model = box_mesh(Vec3::new(0.0, -5.0, 10.0), Vec3::new(10.0, 5.0, 12.0));
        let lip = box_mesh(Vec3::new(10.0, -5.0, 8.5), Vec3::new(11.0, 5.0, 12.0));
        let offset = model.vertices.len() as u32;
        model.vertices.extend_from_slice(&lip.vertices);
        model
            .faces
            .extend(lip.faces.iter().map(|face| face.map(|at| at + offset)));
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());

        assert_eq!(
            neck_of(&placed, Vec3::new(9.7, 0.0, 10.0), &profile()),
            None
        );
        let clear = neck_of(&placed, Vec3::new(5.0, 0.0, 10.0), &profile())
            .expect("away from the lip the underside is open");
        assert!(clear.abs_diff_eq(Vec3::new(5.0, 0.0, 10.0 - profile().top_length_mm()), 1e-4));
    }

    #[test]
    fn a_contact_with_less_room_under_it_than_a_head_is_a_small_pillar_or_nothing() {
        // An underside at z = 10.5 over a bar 9..10: half a millimetre of air, where a
        // 2 mm head would run through the bar.
        let model = under_a_gap_over_a_bar();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let contact = Vec3::new(0.0, 0.0, 10.5);

        let neck = neck_of(&placed, contact, &profile()).expect("a small pillar fits");
        assert!(!has_head(contact, neck), "got a head to {neck}");
        let mut plain = profile();
        plain.small_pillar.enabled = false;
        assert_eq!(
            neck_of(&placed, contact, &plain),
            None,
            "a pillar is wider than its tip"
        );
    }

    #[test]
    fn a_tip_on_a_face_that_looks_up_is_not_placed() {
        // Square on a flat top the head rises, and a trunk from its neck would come
        // down on the contact; hung down instead, it would run through the box.
        let model = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());

        assert_eq!(
            neck_of(&placed, Vec3::new(5.0, 5.0, 10.0), &profile()),
            None
        );
    }

    #[test]
    fn the_pillar_height_is_the_neck_above_the_foot() {
        let profile = profile();
        let columns = columns_on(
            &[SupportPoint::new(Vec3::new(1.0, 1.0, 20.0))],
            &Mesh::default(),
            Transform::default(),
            Profiles::single(&profile),
        );
        let expected = 20.0 - profile.top_length_mm() - profile.base_height_mm();
        let height = columns[0]
            .pillar_height_mm(&profile)
            .expect("the column stands on the plate");
        assert!((height - expected).abs() < 1e-5);
    }
}

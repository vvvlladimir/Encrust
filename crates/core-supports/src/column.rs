use serde::{Deserialize, Serialize};

use crate::group::Profiles;
use crate::placed::Placed;
use core_geometry::{Mat3, Ray, Scalar, Vec2, Vec3, point_triangle, raycast_placed};
use printer_profiles::SupportProfile;
use rayon::prelude::*;

use crate::clear::{Beam, MIN_BODY_MM, clear};
use crate::mesher::FOOT_BEVEL;

/// Plate coordinates put the plate surface at z = 0; see `docs/architecture.md`.
const PLATE_Z_MM: Scalar = 0.0;

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

/// How far a tip leaving a face that does not look down falls per unit it runs out, and the
/// least a face has to lean for the tip to have an out to run along at all.
const OUT_DROP: Scalar = 0.25;
const OUT_MIN: Scalar = 0.05;

/// How far a face a support stands on may lean from horizontal, degrees. Past this the
/// foot has no surface under it: the column runs down the face instead of onto it.
const MAX_LANDING_SLOPE_DEG: Scalar = 45.0;

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
}

impl Landing {
    /// Height at which the pillar body ends: the top of the foot, or the surface it sank
    /// into.
    pub fn pillar_bottom_z(&self, profile: &SupportProfile) -> Scalar {
        if self.on_model {
            // Sunk into what it stands on, so the two cure as one solid.
            self.base.z - profile.landing_depth_mm()
        } else {
            self.base.z + profile.base_height_mm()
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
/// clearance and all, may already touch the steepest face it is let stand on: below that
/// it is sinking into what it stands on, which is what it is meant to do.
pub(crate) fn swept_bottom_z(
    landing: &Landing,
    radius_mm: Scalar,
    profile: &SupportProfile,
) -> Scalar {
    if landing.on_model {
        let lean = MAX_LANDING_SLOPE_DEG.to_radians().tan();
        landing.base.z + (radius_mm + profile.clearance_mm) * lean + SURFACE_EPSILON_MM
    } else {
        landing.pillar_bottom_z(profile)
    }
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
/// what cannot be carried is dropped there.
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
        .map(|(point, support)| {
            let profile = profiles.of(support.group);
            let contact = matrix.transform_point3(support.contact);
            let neck = neck_of(placed, contact, profile);
            Column {
                point,
                group: support.group,
                contact,
                neck,
                landing: landing(placed, neck, profile.pillar_radius_mm(), 1, profile),
            }
        })
        .collect()
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
    let contact = from;
    let ray = Ray::new(contact - Vec3::Z * SURFACE_EPSILON_MM, -Vec3::Z);
    let below = raycast_placed(placed.model, placed.bvh, placed.transform, &ray);

    // Where the body stops being asked about: at the top of the foot on the plate, and on
    // the model at the height its rim already meets the face it stands on.
    let (landing, bottom_z, base_mm) = match below {
        // A foot may not be put down on a blocked face; see `docs/decisions/0093`.
        Some(hit) if placed.blocks_face(hit.face) => return None,
        // The profile forbids standing on the part, so the support leans for the plate
        // instead; see `docs/decisions/0124`.
        Some(hit) if hit.point.z > PLATE_Z_MM && !profile.land_on_model => return None,
        Some(hit) if hit.point.z > PLATE_Z_MM => {
            // The whole beam meets the face it lands on, clearance and all, so the reach
            // that stops being asked about is measured on the beam's own width.
            let touch_mm = (radius_mm + profile.clearance_mm) * lean_of(hit.normal)?;
            (
                Landing {
                    base: hit.point,
                    on_model: true,
                    pad_offset: Vec2::ZERO,
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
            let landing = Landing {
                base,
                on_model: false,
                pad_offset: pad_offset(placed, base, base_mm, profile)?,
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
/// normal of the face it holds, so that the tip meets that face square on rather than at
/// whatever angle the column or the branch under it happens to take (ADR 0125).
///
/// The tip is shortened rather than allowed to eat the room the body needs above the
/// plate, which is what keeps a contact close to the plate standing at all.
pub fn neck_of(placed: &Placed, contact: Vec3, profile: &SupportProfile) -> Vec3 {
    let direction = tip_direction(placed, contact);
    let length_mm = profile.top_length_mm();
    let drop_mm = -direction.z * length_mm;
    let room_mm =
        (contact.z - PLATE_Z_MM - profile.base_height_mm() - MIN_PILLAR_HEIGHT_MM).max(0.0);
    let length_mm = if drop_mm > room_mm {
        length_mm * room_mm / drop_mm
    } else {
        length_mm
    };
    contact + direction * length_mm
}

/// Which way a tip leaves the surface at `contact`: the outward normal of a face that looks
/// down, out and down off one it sits on that does not, so a support placed by hand on any
/// face leaves the model; straight down off a flat top, or from a contact off the surface.
///
/// A contact on an edge or a corner sits on every face meeting there, and leaves the one
/// that looks down most: which of them a search finds first is an accident of the mesh.
fn tip_direction(placed: &Placed, contact: Vec3) -> Vec3 {
    let matrix = placed.transform.to_matrix();
    if matrix.determinant().abs() < SINGULAR {
        return -Vec3::Z;
    }
    let inverse = matrix.inverse();
    let local = inverse.transform_point3(contact);
    let Some(found) = placed.bvh.closest(placed.model, local) else {
        return -Vec3::Z;
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
        return -Vec3::Z;
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
    match (normal.z < 0.0, on_surface) {
        (true, _) => normal,
        (false, true) => out_and_down(normal),
        (false, false) => -Vec3::Z,
    }
}

/// Off a face that does not look down: square off it as nearly as a tip leaving downwards
/// can, its rise traded for a quarter as much fall as it runs out.
fn out_and_down(normal: Vec3) -> Vec3 {
    let across = normal.truncate();
    if across.length() <= OUT_MIN {
        return -Vec3::Z;
    }
    across.extend(-OUT_DROP * across.length()).normalize()
}

/// How many directions and how many reaches a pad is tried at round its trunk before the
/// support has to lean for room instead.
const PAD_DIRECTIONS: u32 = 8;
const PAD_REACHES: u32 = 3;

/// Where the pad of a trunk of `radius_mm` standing at `base` goes: under the trunk, or
/// as little aside as clears the model, and `None` when no place within reach does.
///
/// A pad is never narrowed. It may be moved as far as still leaves the flare inside its
/// top, which only a round pad that does not taper has room for; see
/// `docs/decisions/0239`.
fn pad_offset(
    placed: &Placed,
    base: Vec3,
    radius_mm: Scalar,
    profile: &SupportProfile,
) -> Option<Vec2> {
    if foot_clear(placed, base, profile) {
        return Some(Vec2::ZERO);
    }
    let shape = profile.bottom.shape;
    let inside_mm = profile.flare_lower_radius_mm().max(radius_mm);
    let reach_mm = if shape.is_round() && !shape.tapers() {
        // Inside the rim the pad meets the plate on, which the bevel pulls in.
        profile.base_radius_mm() - profile.base_height_mm() * FOOT_BEVEL - inside_mm
    } else {
        0.0
    };
    if reach_mm <= SURFACE_EPSILON_MM {
        return None;
    }

    (1..=PAD_REACHES)
        .flat_map(|step| (0..PAD_DIRECTIONS).map(move |turn| (step, turn)))
        .map(|(step, turn)| {
            let angle = std::f32::consts::TAU * turn as Scalar / PAD_DIRECTIONS as Scalar;
            Vec2::new(angle.cos(), angle.sin())
                * (reach_mm * step as Scalar / PAD_REACHES as Scalar)
        })
        .find(|offset| foot_clear(placed, base + offset.extend(0.0), profile))
}

/// Whether the foot on the plate has the model out of its way.
///
/// A pad is wider than anything above it, so a column that came down in clear air can
/// still drive its foot into a part that stands on the plate beside it.
pub(crate) fn foot_clear(placed: &Placed, base: Vec3, profile: &SupportProfile) -> bool {
    let radius_mm = profile.base_radius_mm();
    let beam = Beam {
        from: Vec3::new(base.x, base.y, base.z + profile.base_height_mm()),
        to: Vec3::new(base.x, base.y, base.z + SURFACE_EPSILON_MM),
        from_radius_mm: radius_mm,
        to_radius_mm: radius_mm,
    }
    .with_clearance(profile.clearance_mm);
    clear(placed, &beam)
}

/// How much ground the face `normal` belongs to covers per millimetre it rises, or `None`
/// when it is too steep to stand a support on at all.
fn lean_of(normal: Vec3) -> Option<Scalar> {
    let up = normal.z;
    (up >= MAX_LANDING_SLOPE_DEG.to_radians().cos()).then(|| (1.0 - up * up).max(0.0).sqrt() / up)
}

/// Whether `body`, a trunk dropped from its joint, has the model out of its way.
fn stands_clear(placed: &Placed, body: &Beam, profile: &SupportProfile) -> bool {
    if body.from.z - body.to.z < MIN_BODY_MM {
        return true;
    }
    clear(placed, &body.with_clearance(profile.clearance_mm))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{box_mesh, columns_on, landing_on, profile, ramp};
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
        let mut region = crate::Region::default();
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
        let profile = SupportProfile::light();
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
    fn a_pad_with_nowhere_to_go_is_no_landing() {
        // The same step with the trunk 1.3 mm off it: the pad would have to move 3.2 mm,
        // and has 2.3.
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
    fn a_tip_placed_on_a_face_looking_up_leaves_the_model_out_and_down() {
        let model = ramp(4.0, 10.0);
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let profile = profile();

        // The middle of the sloping face, which looks up and out towards -X.
        let contact = Vec3::new(2.0, 0.0, 5.0);
        let along = (neck_of(&placed, contact, &profile) - contact).normalize();
        let normal = Vec3::new(-10.0, 0.0, 4.0).normalize();
        assert!(along.z < 0.0, "the tip runs down, got {along}");
        assert!(
            along.dot(normal) > 0.25,
            "and off the face rather than into the model, got {along}"
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
            let along = (neck_of(&placed, contact, &profile()) - contact).normalize();
            assert!(
                along.dot(underside) > 0.999,
                "of the three faces meeting at {corner} the underside looks down most, \
                 and the tip left along {along}"
            );
        }
    }

    #[test]
    fn a_tip_on_a_flat_top_has_nowhere_but_down() {
        assert_eq!(out_and_down(Vec3::Z), -Vec3::Z);
    }

    #[test]
    fn a_tip_leaves_a_sloping_face_along_its_normal() {
        use core_geometry::Quat;
        use std::f32::consts::PI;

        // The test ramp turned over, so the face that looked up is now an overhang.
        let model = ramp(4.0, 10.0);
        let transform = Transform {
            rotation: Quat::from_rotation_x(PI),
            translation: Vec3::new(0.0, 0.0, 30.0),
            ..Transform::default()
        };
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, transform);
        let profile = profile();

        // The middle of the sloping face, brought out into plate coordinates.
        let contact = transform
            .to_matrix()
            .transform_point3(Vec3::new(2.0, 0.0, 5.0));
        let neck = neck_of(&placed, contact, &profile);
        let along = neck - contact;

        // The face rises 10 over a run of 4, so its normal lies at (-10, 0, 4) before the
        // turn and at (-10, 0, -4) after it.
        let expected = Vec3::new(-10.0, 0.0, -4.0).normalize();
        assert!(
            along.normalize().abs_diff_eq(expected, 1e-4),
            "the tip leaves the face square on, got {}",
            along.normalize()
        );
        assert!(
            (along.length() - profile.top_length_mm()).abs() < 1e-4,
            "the tip is one top segment long"
        );
    }

    #[test]
    fn a_tip_on_a_face_that_looks_up_still_hangs_straight_down() {
        let neck = neck_of(
            &Placed::new(
                &box_mesh(Vec3::ZERO, Vec3::splat(10.0)),
                &Bvh::build(&box_mesh(Vec3::ZERO, Vec3::splat(10.0))),
                Transform::default(),
            ),
            Vec3::new(5.0, 5.0, 10.0),
            &profile(),
        );
        assert!(
            neck.abs_diff_eq(Vec3::new(5.0, 5.0, 10.0 - profile().top_length_mm()), 1e-4),
            "a support is not stood on its head by the face it was dropped on"
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

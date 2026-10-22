use core_geometry::{Mat4, Ray, Scalar, Vec2, Vec3, raycast_placed};
use printer_profiles::SupportProfile;
use serde::{Deserialize, Serialize};

use crate::column::SURFACE_EPSILON_MM;
use crate::placed::Placed;

/// How many ways round its middle a pad is cut back by the model beside it.
pub const PAD_SECTORS: usize = 16;

/// How many heights through the pad the model is looked for at: its sole, its middle and
/// its top, since a part standing on the plate may lean in over it.
const PAD_HEIGHTS: u32 = 3;

/// The least a pad may be cut back to beyond what stands in it, mm, so the cut never
/// bares the flare or the trunk.
const PAD_LEFT_MM: Scalar = 0.05;

/// A pad cut back where the model stands beside it: how far from its middle it reaches in
/// each of [`PAD_SECTORS`] directions across the plate, the first along +x and
/// counter-clockwise from there, mm. See `docs/decisions/0328`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PadCut {
    pub reach_mm: [Scalar; PAD_SECTORS],
}

impl PadCut {
    /// How far the pad reaches towards `angle`, radians from +x, between the two sectors
    /// either side of it, mm.
    pub fn reach_at(&self, angle: Scalar) -> Scalar {
        let at = angle.rem_euclid(std::f32::consts::TAU) / sector_angle();
        let below = (at.floor() as usize) % PAD_SECTORS;
        let above = (below + 1) % PAD_SECTORS;
        let along = at.fract();
        self.reach_mm[below] * (1.0 - along) + self.reach_mm[above] * along
    }

    /// The same cut with the plate turned `angle` radians about the pad's middle.
    #[must_use]
    pub fn turned(&self, angle: Scalar) -> Self {
        Self {
            reach_mm: std::array::from_fn(|sector| {
                self.reach_at(sector as Scalar * sector_angle() - angle)
            }),
        }
    }

    /// The same cut carried through `matrix`: turned with it, its reach left alone as a
    /// pad's width is.
    #[must_use]
    pub(crate) fn moved(&self, matrix: Mat4) -> Self {
        let x = matrix.transform_vector3(Vec3::X);
        self.turned(x.y.atan2(x.x))
    }

    /// `point` of a pad standing at `centre`, pulled in across the plate to the cut.
    pub fn pull_in(&self, centre: Vec3, point: Vec3) -> Vec3 {
        let out = (point - centre).truncate();
        let reach_mm = self.reach_at(out.y.atan2(out.x));
        if out.length() <= reach_mm {
            return point;
        }
        (centre.truncate() + out.normalize() * reach_mm).extend(point.z)
    }
}

fn sector_angle() -> Scalar {
    std::f32::consts::TAU / PAD_SECTORS as Scalar
}

/// The pad standing at `base` cut back to keep its clearance from the model beside it, or
/// `None` where the model leaves it no room past `inside_mm`, the flare and trunk it
/// carries, or does not reach it at all.
pub(crate) fn pad_cut(
    placed: &Placed,
    base: Vec3,
    inside_mm: Scalar,
    profile: &SupportProfile,
) -> Option<PadCut> {
    let rim_mm = profile.base_radius_mm();
    let reach_mm = room_round(placed, base, profile);
    let least_mm = reach_mm.iter().copied().fold(rim_mm, Scalar::min);
    (least_mm >= inside_mm + PAD_LEFT_MM && least_mm < rim_mm).then_some(PadCut { reach_mm })
}

/// How far a pad standing at `base` reaches in each sector before it comes within its
/// clearance of the model, capped at its radius, mm: each sector's edges and middle.
pub(crate) fn room_round(
    placed: &Placed,
    base: Vec3,
    profile: &SupportProfile,
) -> [Scalar; PAD_SECTORS] {
    let rim_mm = profile.base_radius_mm();
    std::array::from_fn(|sector| {
        looks_of(sector)
            .map(|angle| room_towards(placed, base, angle, profile))
            .fold(rim_mm, Scalar::min)
    })
}

/// Whether a pad standing at `base` keeps its clearance from the model all round, asked
/// the way [`room_round`] is and stopping at the first look that finds the model.
pub(crate) fn pad_clear(placed: &Placed, base: Vec3, profile: &SupportProfile) -> bool {
    let rim_mm = profile.base_radius_mm();
    (0..PAD_SECTORS)
        .flat_map(looks_of)
        .all(|angle| room_towards(placed, base, angle, profile) >= rim_mm)
}

/// The directions sector `sector` is looked along, radians from +x: its two edges and its
/// middle, so nothing between two looks pokes far into the pad.
fn looks_of(sector: usize) -> impl Iterator<Item = Scalar> {
    let middle = sector as Scalar * sector_angle();
    let half = sector_angle() / 2.0;
    [middle - half, middle, middle + half].into_iter()
}

/// How far a pad standing at `base` reaches towards `angle` before it comes within its
/// clearance of the model at any height through it, capped at its radius, mm.
///
/// Looked for out from the pad's middle, so a toe standing inside the pad is found
/// wherever it is.
fn room_towards(placed: &Placed, base: Vec3, angle: Scalar, profile: &SupportProfile) -> Scalar {
    let rim_mm = profile.base_radius_mm();
    let height_mm = profile.base_height_mm();
    let out = Vec2::new(angle.cos(), angle.sin()).extend(0.0);
    (0..PAD_HEIGHTS)
        .map(|step| {
            let up_mm = SURFACE_EPSILON_MM
                + (height_mm - SURFACE_EPSILON_MM) * step as Scalar / (PAD_HEIGHTS - 1) as Scalar;
            let ray = Ray::new(base + Vec3::Z * up_mm, out);
            match raycast_placed(placed.model, placed.bvh, placed.transform, &ray) {
                // Set off inside the model: there is no pad here at all.
                Some(hit) if hit.normal.dot(out) > 0.0 => 0.0,
                Some(hit) => hit.distance - profile.clearance_mm,
                None => rim_mm,
            }
        })
        .fold(rim_mm, Scalar::min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{box_mesh, profile};
    use core_geometry::{Bvh, Transform};

    fn full(reach_mm: Scalar) -> PadCut {
        PadCut {
            reach_mm: [reach_mm; PAD_SECTORS],
        }
    }

    #[test]
    fn a_pad_beside_a_wall_is_cut_back_on_that_side_alone() {
        // A part on the plate whose side stands at x = 3, beside a pad 8 mm across at the
        // origin: the side facing it keeps its 0.5 mm clearance, 2.5 mm out.
        let model = box_mesh(Vec3::new(3.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 5.0));
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let cut = pad_cut(&placed, Vec3::ZERO, 1.1, &profile()).expect("room is left");
        assert!(
            (cut.reach_at(0.0) - 2.5).abs() < 1e-3,
            "towards the wall it stops its clearance short, got {}",
            cut.reach_at(0.0)
        );
        assert_eq!(
            cut.reach_at(std::f32::consts::PI),
            4.0,
            "away from it the pad keeps its whole radius"
        );
    }

    #[test]
    fn a_pad_the_model_leaves_no_room_round_its_trunk_is_not_cut() {
        let model = box_mesh(Vec3::new(1.2, -10.0, 0.0), Vec3::new(10.0, 10.0, 5.0));
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        assert!(pad_cut(&placed, Vec3::ZERO, 1.1, &profile()).is_none());
    }

    #[test]
    fn a_point_past_the_cut_is_pulled_in_to_it_and_one_inside_stays() {
        let cut = full(2.0);
        let centre = Vec3::new(1.0, 1.0, 0.0);
        let pulled = cut.pull_in(centre, Vec3::new(5.0, 1.0, 0.3));
        assert!(
            pulled.abs_diff_eq(Vec3::new(3.0, 1.0, 0.3), 1e-5),
            "got {pulled}"
        );
        let inside = Vec3::new(2.0, 1.0, 0.3);
        assert_eq!(cut.pull_in(centre, inside), inside);
    }

    #[test]
    fn turning_a_cut_a_whole_sector_moves_each_reach_one_on() {
        let mut cut = full(4.0);
        cut.reach_mm[0] = 2.0;
        let turned = cut.turned(sector_angle());
        assert!(
            (turned.reach_mm[1] - 2.0).abs() < 1e-5 && (turned.reach_mm[0] - 4.0).abs() < 1e-5,
            "the cut sector moved one on, got {:?}",
            turned.reach_mm
        );
    }
}

use super::Style;
use super::tube::{RING_EPSILON_MM, Ring, Tube, push, round_off};
use crate::base::skate_half_width_mm;
use crate::pad::PAD_SECTORS;
use crate::tree::SupportTree;
use core_geometry::{Scalar, Vec3};
use printer_profiles::{ContactShape, SupportProfile};

/// How far a tapering foot narrows on the way up to the support it carries: it never
/// closes in past this fraction of its own radius, so the rim stays a rim.
pub const FOOT_TAPER: Scalar = 0.45;

/// How much of a tapering pad's height its bevel at the plate takes, as a fraction; above it
/// the pad narrows towards the support (ADR 0044, 0130).
pub(super) const FOOT_BEVEL: Scalar = 0.6;

/// How far the pad's wall drops and pulls in at the plate, mm: forty-five degrees off it, over
/// the whole height of a straight pad and `FOOT_BEVEL` of a tapering one (ADR 0432).
pub fn foot_bevel_mm(profile: &SupportProfile) -> Scalar {
    let height = profile.bottom.platform_thickness_mm;
    if profile.bottom.shape.tapers() {
        height * FOOT_BEVEL
    } else {
        height
    }
}

/// How far a trunk reaches into the foot under it, as a fraction of the foot's height: enough
/// to overlap, not enough to show through the bottom of the pad.
pub(super) const FOOT_BITE: Scalar = 0.5;

/// Closes a trunk standing on the model with its tip upside down; `length` runs from the root
/// to the sunk bottom, and a sphere answers true when it bites to a point (ADR 0124, 0243).
pub(super) fn foot_tip(
    rings: &mut Vec<Ring>,
    length: Scalar,
    body_mm: Scalar,
    profile: &SupportProfile,
) -> bool {
    let pointed = profile.tip.shape == ContactShape::Sphere;
    let end_mm = if pointed {
        (length - profile.landing_depth_mm()).max(0.0)
    } else {
        length
    };
    let taper_mm = profile.top.length_mm.min(end_mm);
    let contact_mm = profile.contact_radius_mm().min(body_mm);

    push(rings, end_mm - taper_mm, body_mm);
    push(rings, end_mm, contact_mm);
    if !pointed {
        round_off(rings, end_mm, contact_mm);
    }
    pointed
}

/// How wide the trunk is where it meets what it stands on, millimetres: its own width on
/// the model, where it ends in a tip like any branch; see `docs/decisions/0242`.
pub(super) fn trunk_base_mm(tree: &SupportTree, profile: &SupportProfile, style: Style) -> Scalar {
    let root = tree.root();
    if tree.landing().on_model {
        return style.body_radius_mm(root.radius_mm, profile);
    }
    let height = (root.position - trunk_bottom(tree, profile)).length();
    let base = profile.trunk_base_radius_mm(height, tree.tip_count());
    style.body_radius_mm(base.max(root.radius_mm), profile)
}

/// Where the trunk under the root comes to rest, in plate coordinates: on the face itself for
/// a small pillar, whose half ball takes the depth along its own axis (ADR 0340).
pub(super) fn trunk_bottom(tree: &SupportTree, profile: &SupportProfile) -> Vec3 {
    let landing = tree.landing();
    if landing.on_model && !landing.on_support && Style::of(tree, profile).small {
        return landing.base;
    }
    Vec3::new(
        landing.base.x,
        landing.base.y,
        landing.pillar_bottom_z(profile),
    )
}

/// The foot the trunk stands on, as a solid of its own so that it can have a different
/// number of sides from the support above it. `None` when the support lands on the model.
pub(super) fn foot(tree: &SupportTree, profile: &SupportProfile, style: Style) -> Option<Tube> {
    let landing = tree.landing();
    if landing.on_model || landing.in_plate {
        return None;
    }

    let shape = profile.bottom.shape;
    let rim_mm = profile.base_radius_mm();
    let trunk_mm = trunk_base_mm(tree, profile, style);
    let top_mm = foot_top_radius_mm(profile, trunk_mm);

    let height = profile.bottom.platform_thickness_mm;
    let bevel = foot_bevel_mm(profile);
    // The pad stands where the landing found room for it, which is under the trunk unless
    // the model was in the way; see `docs/decisions/0239`.
    let centre = landing.pad_centre();
    let top = Vec3::new(centre.x, centre.y, landing.pillar_bottom_z(profile));
    // A cut pad is drawn with a side to every half sector, so the cut keeps its shape.
    let facets = if landing.pad_cut.is_some() {
        profile.facets.max(2 * PAD_SECTORS as u32)
    } else {
        profile.facets
    };

    // The bevel is what a blade gets under: the pad meets the plate on a rim pulled in
    // from its widest point. See `docs/decisions/0044` and `docs/decisions/0130`.
    let mut rings = Vec::with_capacity(3);
    push(&mut rings, 0.0, top_mm);
    if shape.tapers() {
        push(&mut rings, height - bevel, rim_mm);
    }
    push(&mut rings, height, plate_radius_mm(profile, trunk_mm));

    Some(Tube {
        top,
        direction: -Vec3::Z,
        rings,
        sides: shape.sides(facets) as usize,
        apex: None,
        tail: None,
    })
}

/// Radius of the ring the pad meets the plate on, millimetres: its rim pulled in by the
/// bevel, and never inside the trunk standing in it.
pub(super) fn plate_radius_mm(profile: &SupportProfile, trunk_mm: Scalar) -> Scalar {
    (profile.base_radius_mm() - foot_bevel_mm(profile)).max(trunk_mm)
}

/// How many steps the fillet between a pillar and its pad is drawn in.
pub(super) const FLARE_RINGS: u32 = 4;

/// The fillet widening the pillar out into the pad or raft, or `None` on the model or with no
/// room, drawn with the pillar's facets (ADR 0130, 0341).
pub(super) fn flare(tree: &SupportTree, profile: &SupportProfile, style: Style) -> Option<Tube> {
    let landing = tree.landing();
    let trunk_mm = trunk_base_mm(tree, profile, style);
    // Inside the pad at every depth it reaches, so it never shows through its side.
    let widest_mm = if profile.raft.enabled {
        Scalar::INFINITY
    } else if profile.bottom.shape.is_round() {
        plate_radius_mm(profile, trunk_mm).min(foot_top_radius_mm(profile, trunk_mm))
    } else {
        skate_half_width_mm(profile)
    };
    let lower_mm = profile.flare_lower_radius_mm().min(widest_mm).max(trunk_mm);

    let bottom_z = landing.pillar_bottom_z(profile);
    let root_z = tree.root().position.z;
    // Only the pillar under a head is filleted, never the head itself.
    let head_mm = if tree.root().is_leaf() && !style.small {
        profile.top.length_mm
    } else {
        0.0
    };
    let room_mm = (root_z - bottom_z - head_mm).max(0.0);
    // The trunk narrows going up, so the fillet meets it where it is that much narrower:
    // r(top) + rise = lower with r falling `narrowing` per mm, and never past the room.
    let narrowing = (trunk_mm - super::trunk_radius_at_mm(tree, profile, root_z))
        / (root_z - bottom_z).max(RING_EPSILON_MM);
    let rise_mm = ((lower_mm - trunk_mm) / (1.0 - narrowing).max(RING_EPSILON_MM)).min(room_mm);
    if rise_mm < RING_EPSILON_MM {
        return None;
    }
    let upper_mm = super::trunk_radius_at_mm(tree, profile, bottom_z + rise_mm);

    // A quarter circle, tangent to the pillar at its top and to the pad at its foot, so
    // the joint carries no notch for the peel to start a crack in; see ADR 0341.
    let mut rings = Vec::with_capacity(FLARE_RINGS as usize + 2);
    for step in 0..=FLARE_RINGS {
        let angle = std::f32::consts::FRAC_PI_2 * step as Scalar / FLARE_RINGS as Scalar;
        push(
            &mut rings,
            rise_mm * angle.sin(),
            rise_mm.mul_add(1.0 - angle.cos(), upper_mm),
        );
    }
    push(
        &mut rings,
        rise_mm + profile.footing_height_mm() * FOOT_BITE,
        upper_mm + rise_mm,
    );

    Some(Tube {
        top: Vec3::new(landing.base.x, landing.base.y, bottom_z + rise_mm),
        direction: -Vec3::Z,
        rings,
        sides: profile.facets as usize,
        apex: None,
        tail: None,
    })
}

/// Radius of what a support standing on the plate meets the ground with, millimetres: its
/// pad's rim, or the bottom of its flare where a raft takes the pad's place.
pub(crate) fn footing_radius_mm(tree: &SupportTree, profile: &SupportProfile) -> Scalar {
    if !profile.raft.enabled {
        return profile.base_radius_mm();
    }
    let trunk_mm = trunk_base_mm(tree, profile, Style::of(tree, profile));
    profile.flare_lower_radius_mm().max(trunk_mm)
}

/// Radius of the pad's top ring, mm: its rim or the narrowed one of a tapering foot, still wider
/// than the trunk in it or the trunk shows through its side.
pub(super) fn foot_top_radius_mm(profile: &SupportProfile, trunk_mm: Scalar) -> Scalar {
    let rim_mm = profile.base_radius_mm();
    if profile.bottom.shape.tapers() {
        (rim_mm * FOOT_TAPER).max(trunk_mm)
    } else {
        rim_mm
    }
}

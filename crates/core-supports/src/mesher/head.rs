use super::tube::{Ring, ball, dome, push, sweep};
use super::{Style, strut};
use crate::Landing;
use crate::column::{SURFACE_EPSILON_MM, pin_mm};
use crate::placed::Placed;
use crate::tree::{SupportTree, TreeNode};
use core_geometry::{Mesh, Ray, Scalar, Vec2, Vec3};
use printer_profiles::{ContactShape, SupportProfile};

/// How many rings a sphere tip is drawn with, as a fraction of the facets: under a millimetre
/// across, it needs far fewer rings than sides.
pub(super) const SPHERE_RINGS: u32 = 6;

/// The tip alone, from `contact` out to `neck`: what a support that cannot stand is shown
/// as before it is put down.
pub(crate) fn mesh_head(contact: Vec3, neck: Vec3, profile: &SupportProfile) -> Mesh {
    let radius_mm = profile.pillar_radius_mm();
    let nodes = vec![
        TreeNode {
            position: contact,
            radius_mm,
            parent: Some(1),
            point: Some(0),
            head: None,
        },
        TreeNode {
            position: neck,
            radius_mm,
            parent: None,
            point: None,
            head: None,
        },
    ];
    let landing = Landing {
        base: neck,
        on_model: false,
        pad_offset: Vec2::ZERO,
        pad_cut: None,
        on_support: false,
    };
    let tree = SupportTree::new(nodes, 1, landing);
    let mut mesh = Mesh::default();
    sweep(
        &mut mesh,
        &strut(
            &tree,
            0,
            1,
            profile,
            Style {
                small: false,
                bite_limit_mm: Scalar::INFINITY,
                face: None,
                ground: None,
            },
        ),
    );
    mesh
}

/// The rings a tube along `direction` starts with: the tip and widening top segment at a contact,
/// else the joint's width; the top segment no longer than `length`, so it does not run through the pad.
pub(super) fn head(
    rings: &mut Vec<Ring>,
    node: &TreeNode,
    direction: Vec3,
    profile: &SupportProfile,
    style: Style,
    length: Scalar,
) {
    if !node.is_leaf() {
        dome(rings, style.body_radius_mm(node.radius_mm, profile));
        return;
    }

    let contact_mm = profile.contact_radius_mm();
    let top_mm = profile.top.length_mm.min(length);
    let body_mm = style.body_radius_mm(node.radius_mm, profile);
    // A small pillar is one width throughout, its end the half ball a plane tip ends in (ADR 0340).
    if style.small {
        cap(rings, -cap_centre_mm(style, profile, length), body_mm);
        return;
    }
    if profile.tip.shape == ContactShape::Sphere {
        ball(rings, style.bite_mm(profile), contact_mm, SPHERE_RINGS);
        // One cone from the contact out to the body's own width; see `docs/decisions/0238`.
        push(rings, top_mm, body_mm);
        return;
    }

    // A half ball of the contact's width on the contact, pushed in by the bite, so the face cuts
    // it in a circle of the contact's width; a pin keeps a leaning cone off it (ADR 0339).
    let centre_mm = cap_centre_mm(style, profile, length);
    cap(rings, -centre_mm, contact_mm);
    if let Some(face) = style.face {
        push(rings, pin_mm(contact_mm, direction, face), contact_mm);
    }
    push(rings, top_mm - centre_mm, body_mm);
}

/// How far into the part a plane tip's or a small pillar's half ball is centred on a tube
/// `length` long, millimetres: its bite, held to the half wall behind it and to the room it has.
pub(super) fn cap_centre_mm(style: Style, profile: &SupportProfile, length: Scalar) -> Scalar {
    let (radius_mm, room_mm) = if style.small {
        (profile.small_pillar.radius_mm(), length)
    } else {
        (
            profile.contact_radius_mm(),
            profile.top.length_mm.min(length),
        )
    };
    style
        .bite_mm(profile)
        .min(style.bite_limit_mm - radius_mm)
        .min(room_mm)
}

/// The rings of a half ball of `radius_mm` centred `centre_mm` along the axis, down to its
/// full width there; its pole, `radius_mm` further back, is the tube's apex.
pub(super) fn cap(rings: &mut Vec<Ring>, centre_mm: Scalar, radius_mm: Scalar) {
    for step in 1..=CAP_RINGS {
        let angle = std::f32::consts::FRAC_PI_2 * step as Scalar / CAP_RINGS as Scalar;
        push(
            rings,
            centre_mm - radius_mm * angle.cos(),
            radius_mm * angle.sin(),
        );
    }
}

/// How many rings the half ball a plane tip ends in is drawn with.
pub(super) const CAP_RINGS: u32 = 4;

/// The outward normal of the face of the model `landing` stands on, if it stands on the
/// model itself.
pub(crate) fn ground_normal(placed: &Placed, landing: Landing) -> Option<Vec3> {
    if !landing.on_model || landing.on_support {
        return None;
    }
    let ray = Ray::new(landing.base + Vec3::Z * SURFACE_EPSILON_MM, -Vec3::Z);
    placed
        .raycast(&ray)
        .filter(|hit| hit.distance <= 2.0 * SURFACE_EPSILON_MM)
        .map(|hit| hit.normal)
}

/// Where a tube `length` long pokes into the model at a contact, back up its own axis,
/// or the pole of the dome at a joint (ADR 0125, 0339, 0340).
pub(super) fn apex_of(
    node: &TreeNode,
    direction: Vec3,
    profile: &SupportProfile,
    style: Style,
    length: Scalar,
) -> Vec3 {
    let body_mm = style.body_radius_mm(node.radius_mm, profile);
    let depth_mm = if !node.is_leaf() {
        body_mm
    } else if style.small {
        cap_centre_mm(style, profile, length) + body_mm
    } else if profile.tip.shape == ContactShape::Plane {
        cap_centre_mm(style, profile, length) + profile.contact_radius_mm()
    } else {
        style.bite_mm(profile)
    };
    node.position - direction * depth_mm
}

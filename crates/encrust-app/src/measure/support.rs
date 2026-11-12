//! Where a click of the Measure tool aimed at a support is caught.

use core_geometry::{Scalar, Vec3};

use super::{Snap, Snapped, on_segment};

/// The point a click by `near` takes on the support part aimed along `axis`, plate mm: a
/// support has no faces, so a corner is an end of its axis and an edge the axis itself.
pub fn snap_to_support((from, to): (Vec3, Vec3), near: Vec3, snap: Snap) -> (Vec3, Snapped) {
    let on_axis = on_segment(near, from, to);
    match snap {
        Snap::Free => (on_axis, Snapped::Free),
        _ if (to - from).length_squared() <= Scalar::EPSILON => (from, Snapped::Corner),
        Snap::Corner if near.distance(from) <= near.distance(to) => (from, Snapped::Corner),
        Snap::Corner => (to, Snapped::Corner),
        Snap::Edge => (on_axis, Snapped::Edge),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strut from 10 mm up to 30 mm, standing on (0, 0).
    const STRUT: (Vec3, Vec3) = (Vec3::new(0.0, 0.0, 30.0), Vec3::new(0.0, 0.0, 10.0));

    #[test]
    fn a_click_beside_a_strut_lands_on_its_axis_or_its_nearer_end() {
        let near = Vec3::new(1.0, 0.0, 14.0);
        assert_eq!(
            snap_to_support(STRUT, near, Snap::Edge),
            (Vec3::new(0.0, 0.0, 14.0), Snapped::Edge),
            "square across to the axis at 14 mm"
        );
        assert_eq!(
            snap_to_support(STRUT, near, Snap::Corner),
            (Vec3::new(0.0, 0.0, 10.0), Snapped::Corner),
            "14 mm is nearer the end at 10 mm than the one at 30 mm"
        );
    }

    #[test]
    fn a_tip_is_its_own_point_unless_placed_freely() {
        let tip = Vec3::new(5.0, 5.0, 20.0);
        let near = Vec3::new(5.2, 5.0, 20.1);
        assert_eq!(
            snap_to_support((tip, tip), near, Snap::Edge),
            (tip, Snapped::Corner)
        );
        assert_eq!(
            snap_to_support((tip, tip), near, Snap::Free),
            (tip, Snapped::Free),
            "a point has nowhere else to place a click"
        );
    }
}

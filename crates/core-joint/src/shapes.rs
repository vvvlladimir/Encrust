use core_geometry::{Mesh, Scalar, Vec2, Vec3, revolve, segments_for};

/// Furthest a round side's flats stray from the true circle, mm: under what a panel's
/// pixel resolves.
const ROUND_MM: Scalar = 0.01;

/// Fewest flats round a peg, so a very thin one is still round enough to turn in.
const FEWEST_SIDES: usize = 24;

/// Where a peg's side is cut flat, as a share of its radius: the gap it leaves, some
/// 0.3 mm on a 4 mm peg, lets air and spare glue out of a blind socket.
const VENT: Scalar = 0.85;

/// A solid of revolution about `axis` from `base`: one ring for each `(along_mm,
/// radius_mm)`, joined in order and closed at both ends, wound outward.
///
/// `vented` cuts one side of every ring flat; the profile stays convex either way.
fn revolved(base: Vec3, axis: Vec3, rings: &[(Scalar, Scalar)], vented: bool) -> Mesh {
    let widest = rings.iter().map(|ring| ring.1).fold(0.0, Scalar::max);
    let sides = segments_for(widest, ROUND_MM).max(FEWEST_SIDES);
    let outline: Vec<Vec2> = (0..sides)
        .map(|step| {
            let around = Vec2::from_angle(std::f32::consts::TAU * step as Scalar / sides as Scalar);
            if vented {
                Vec2::new(around.x.min(VENT), around.y)
            } else {
                around
            }
        })
        .collect();
    revolve(base, axis, rings, &outline)
}

/// The chamfer on a peg's ends, mm, which leads it in and clears the flare of a
/// socket's first layers.
fn chamfer(radius_mm: Scalar) -> Scalar {
    (0.25 * radius_mm).min(0.4)
}

/// A pin standing on a half at `foot`, along `axis`: `sink_mm` of it inside the half so
/// the two are one solid, `length_mm` out of it, its tip chamfered and its side vented.
pub(crate) fn pin(
    foot: Vec3,
    axis: Vec3,
    radius_mm: Scalar,
    length_mm: Scalar,
    sink_mm: Scalar,
) -> Mesh {
    let tip = chamfer(radius_mm);
    let rings = [
        (-sink_mm, radius_mm),
        (length_mm - tip, radius_mm),
        (length_mm, radius_mm - tip),
    ];
    revolved(foot, axis, &rings, true)
}

/// A loose dowel standing on the plate at the origin, chamfered at both ends and vented.
pub(crate) fn dowel(radius_mm: Scalar, length_mm: Scalar) -> Mesh {
    let end = chamfer(radius_mm);
    let rings = [
        (0.0, radius_mm - end),
        (end, radius_mm),
        (length_mm - end, radius_mm),
        (length_mm, radius_mm - end),
    ];
    revolved(Vec3::ZERO, Vec3::Z, &rings, true)
}

/// What a socket takes out of a half from `mouth` along `axis`, `depth_mm` deep, its
/// mouth widened by a 45° chamfer. It starts `clear_mm` outside the half, so its end is
/// not on the half's face, where a boolean would have to decide two coincident faces.
pub(crate) fn socket(
    mouth: Vec3,
    axis: Vec3,
    radius_mm: Scalar,
    depth_mm: Scalar,
    clear_mm: Scalar,
) -> Mesh {
    let flare = (0.2 * radius_mm).min(0.3);
    let rings = [
        (-clear_mm, radius_mm + flare + clear_mm),
        (flare, radius_mm),
        (depth_mm, radius_mm),
    ];
    revolved(mouth, axis, &rings, false)
}

/// A plain round bar from `from` along `axis` for `length_mm`.
pub(crate) fn bar(from: Vec3, axis: Vec3, radius_mm: Scalar, length_mm: Scalar) -> Mesh {
    revolved(
        from,
        axis,
        &[(0.0, radius_mm), (length_mm, radius_mm)],
        false,
    )
}

#[cfg(test)]
mod tests {
    use core_geometry::{diagnose, signed_volume};

    use super::*;

    fn closed(mesh: &Mesh, what: &str) -> Scalar {
        let report = diagnose(mesh);
        assert_eq!(report.boundary_edges, 0, "{what} is open: {report:?}");
        let volume = signed_volume(mesh);
        assert!(volume > 0.0, "{what} is wound inward: {volume}");
        volume
    }

    #[test]
    fn a_bar_has_the_volume_of_its_cylinder() {
        let volume = closed(&bar(Vec3::ZERO, Vec3::X, 2.0, 5.0), "a bar");
        // The flats come within 0.01 mm of the circle, so the area is short by under 1%.
        let expected = std::f32::consts::PI * 4.0 * 5.0;
        assert!(
            (volume - expected).abs() / expected < 0.01,
            "pi r2 h is {expected}, got {volume}"
        );
    }

    #[test]
    fn a_vented_pin_is_a_closed_solid_a_little_under_its_cylinder() {
        let volume = closed(&pin(Vec3::ZERO, Vec3::Z, 2.0, 5.0, 0.1), "a pin");
        let cylinder = std::f32::consts::PI * 4.0 * 5.1;
        assert!(
            volume < cylinder && volume > 0.9 * cylinder,
            "the vent and the chamfer take a few percent off {cylinder}, got {volume}"
        );
    }

    #[test]
    fn dowels_and_sockets_close_too() {
        closed(&dowel(2.0, 8.0), "a dowel");
        closed(&socket(Vec3::ZERO, -Vec3::Z, 2.1, 4.0, 0.5), "a socket");
    }
}

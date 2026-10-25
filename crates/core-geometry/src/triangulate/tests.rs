use super::*;

/// How often each undirected edge of the fill is used.
fn edge_uses(triangles: &[[u32; 3]]) -> FastMap<(u32, u32), usize> {
    let mut uses: FastMap<(u32, u32), usize> = FastMap::default();
    for triangle in triangles {
        for corner in 0..3 {
            let (a, b) = (triangle[corner], triangle[(corner + 1) % 3]);
            *uses.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    uses
}

/// Every edge of the rings is covered once, which is what the wall on the other side of
/// it needs, and nothing the fill added is left on its own.
fn covers(rings: &[&[u32]], triangles: &[[u32; 3]]) {
    let uses = edge_uses(triangles);
    let mut sides: FastSet<(u32, u32)> = FastSet::default();
    for ring in rings {
        for corner in 0..ring.len() {
            let (a, b) = (ring[corner], ring[(corner + 1) % ring.len()]);
            if a == b {
                continue;
            }
            let key = (a.min(b), a.max(b));
            assert_eq!(uses.get(&key), Some(&1), "the ring edge {key:?} is covered");
            sides.insert(key);
        }
    }
    for (key, count) in uses {
        if !sides.contains(&key) {
            assert_eq!(count, 2, "the edge {key:?} the fill added is used twice");
        }
    }
}

#[test]
fn a_triangle_with_no_area_is_flipped_into_the_one_beside_it() {
    // Corner 1 stands halfway along the edge from corner 0 to corner 2, so the ear
    // over the three of them has no area.
    let flat = [
        Vec2::new(0.0, 0.0),
        Vec2::new(5.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(5.0, 10.0),
    ];
    let mut triangles = [[0, 1, 2], [0, 2, 3]];

    flip_flat(&mut triangles, &flat);

    for triangle in &triangles {
        let points = triangle.map(|corner| flat[corner as usize]);
        let area = (points[1] - points[0]).perp_dot(points[2] - points[0]);
        assert!(area != 0.0, "{triangle:?} has area, got {area}");
    }
    covers(&[&[0, 1, 2, 3]], &triangles);
}

#[test]
fn a_hole_whose_side_runs_on_from_a_side_of_the_ring_is_still_covered() {
    // The hole's bottom lies on the line the ring's notch runs along, the line ear
    // clipping bridges the hole along, and the corners on it collapsed.
    let flat = [
        Vec2::new(0.0, -10.0),
        Vec2::new(1.0, -10.0),
        Vec2::new(2.0, -10.0),
        Vec2::new(3.0, -10.0),
        Vec2::new(5.0, -10.0),
        Vec2::new(5.0, -15.0),
        Vec2::new(30.0, -15.0),
        Vec2::new(30.0, 15.0),
        Vec2::new(0.0, 15.0),
        Vec2::new(20.0, -10.0),
        Vec2::new(20.0, 5.0),
        Vec2::new(25.0, 5.0),
        Vec2::new(25.0, -10.0),
        Vec2::new(22.5, -10.0),
    ];
    let ring = [0, 1, 2, 3, 4, 5, 6, 7, 8];
    let hole = vec![9, 10, 11, 12, 13];

    let triangles = triangulate(&ring, &[&hole], &flat);

    covers(&[&ring, &hole], &triangles);
}

#[test]
fn a_corner_standing_on_the_one_before_it_is_still_covered() {
    // Corner 3 is corner 2 over again, as a section has wherever the model touched
    // itself on the plane.
    let flat = [
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(10.0, 10.0),
        Vec2::new(10.0, 10.0),
        Vec2::new(0.0, 10.0),
    ];
    let ring = [0, 1, 2, 3, 4];

    let triangles = triangulate(&ring, &[], &flat);

    covers(&[&ring], &triangles);
}

#[test]
fn a_run_of_corners_on_one_place_is_covered_edge_by_edge() {
    // Corners 3 and 4 both stand on corner 2, as a section has where a cut runs through
    // a vertex several of its edges share.
    let flat = [
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(10.0, 10.0),
        Vec2::new(10.0, 10.0),
        Vec2::new(10.0, 10.0),
        Vec2::new(0.0, 10.0),
    ];
    let ring = [0, 1, 2, 3, 4, 5];

    let triangles = triangulate(&ring, &[], &flat);

    covers(&[&ring], &triangles);
}

#[test]
fn a_run_on_the_first_corner_that_closes_the_ring_is_covered() {
    // Corners 4 and 5 stand on corner 0, so the run wraps round the ring's end.
    let flat = [
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(10.0, 10.0),
        Vec2::new(0.0, 10.0),
        Vec2::new(0.0, 0.0),
        Vec2::new(0.0, 0.0),
    ];
    let ring = [0, 1, 2, 3, 4, 5];

    let triangles = triangulate(&ring, &[], &flat);

    covers(&[&ring], &triangles);
}

#[test]
fn the_fill_winds_as_the_outer_ring_whichever_way_that_runs() {
    let flat = [
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(10.0, 10.0),
        Vec2::new(0.0, 10.0),
    ];
    for (ring, anticlockwise) in [([0, 1, 2, 3], true), ([3, 2, 1, 0], false)] {
        for [a, b, c] in triangulate(&ring, &[], &flat) {
            let (a, b, c) = (flat[a as usize], flat[b as usize], flat[c as usize]);
            assert_eq!(
                (b - a).perp_dot(c - a) > 0.0,
                anticlockwise,
                "every triangle turns the way the ring does"
            );
        }
    }
}

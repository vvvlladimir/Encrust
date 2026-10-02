use core_geometry::{Mesh, Scalar, Vec2};

/// Identity of the mesh edge a contour point sits on: both vertex indices, lower first.
///
/// Two faces sharing an edge produce the same key, which is what lets contours be
/// stitched by topology instead of by comparing coordinates.
pub(crate) type EdgeKey = (u32, u32);

/// Where one face crosses a slicing plane, directed so that material lies to the left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Crossing {
    pub start_edge: EdgeKey,
    pub end_edge: EdgeKey,
    pub start: Vec2,
    pub end: Vec2,
}

/// Intersects one face with the plane at height `z`, in millimetres.
///
/// A vertex sitting exactly on the plane counts as below it. That single rule removes
/// every degenerate case at once; see `docs/design/slicing.md`.
pub(crate) fn crossing(mesh: &Mesh, face: usize, z: Scalar) -> Option<Crossing> {
    let indices = *mesh.faces.get(face)?;
    let mut above = [false; 3];
    for (corner, index) in indices.iter().enumerate() {
        above[corner] = mesh.vertices.get(*index as usize)?.z > z;
    }

    // Around a triangle the classification can flip either zero or two times, never one,
    // so a crossing face always yields exactly one entry and one exit edge.
    let mut start_edge = None;
    let mut end_edge = None;
    for corner in 0..3 {
        let next = (corner + 1) % 3;
        if above[corner] == above[next] {
            continue;
        }
        let edge = canonical(indices[corner], indices[next]);
        // Walking a face down through the plane opens the contour, walking up closes it.
        // That is what makes outer contours come out counter-clockwise.
        if above[corner] {
            start_edge = Some(edge);
        } else {
            end_edge = Some(edge);
        }
    }

    let (start_edge, end_edge) = (start_edge?, end_edge?);
    Some(Crossing {
        start_edge,
        end_edge,
        start: point_on(mesh, start_edge, z)?,
        end: point_on(mesh, end_edge, z)?,
    })
}

fn canonical(a: u32, b: u32) -> EdgeKey {
    if a <= b { (a, b) } else { (b, a) }
}

/// Interpolates the plane crossing along `edge`, always from the lower vertex index to
/// the higher one so that both faces sharing the edge land on bit-identical coordinates.
fn point_on(mesh: &Mesh, edge: EdgeKey, z: Scalar) -> Option<Vec2> {
    let lo = *mesh.vertices.get(edge.0 as usize)?;
    let hi = *mesh.vertices.get(edge.1 as usize)?;
    let (d_lo, d_hi) = (lo.z - z, hi.z - z);
    // The caller only reaches here for an edge whose ends straddle the plane, so one
    // height is strictly positive and the other is not: the difference cannot be zero.
    let t = d_lo / (d_lo - d_hi);
    Some(lo.truncate() + t * (hi.truncate() - lo.truncate()))
}

#[cfg(test)]
mod tests {
    use core_geometry::Vec3;

    use super::*;

    /// Two faces of a box wall at x = 1, wound outwards, sharing the diagonal edge 1-2.
    fn wall() -> Mesh {
        Mesh::new(
            vec![
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(1.0, 0.0, 1.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        )
    }

    #[test]
    fn an_outward_wall_is_walked_with_material_on_the_left() {
        let found = crossing(&wall(), 0, 0.5).expect("the face spans the plane");
        // The wall faces +X, so its cross-section must run towards +Y for the enclosing
        // contour to come out counter-clockwise.
        assert!(found.end.y > found.start.y);
        assert!((found.start.x - 1.0).abs() < 1e-6);
    }

    #[test]
    fn faces_sharing_an_edge_agree_bit_for_bit() {
        let mesh = wall();
        let first = crossing(&mesh, 0, 0.5).expect("crosses");
        let second = crossing(&mesh, 1, 0.5).expect("crosses");

        assert_eq!(first.start_edge, second.end_edge);
        assert_eq!(
            first.start, second.end,
            "a shared edge yields one exact point"
        );
    }

    #[test]
    fn a_vertex_exactly_on_the_plane_produces_no_crossing() {
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
        );
        // The apex touches z = 1 and the base sits below it: nothing crosses.
        assert!(crossing(&mesh, 0, 1.0).is_none());
    }

    #[test]
    fn an_edge_lying_in_the_plane_is_the_crossing_itself() {
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 1.0),
            ],
            vec![[0, 1, 2]],
        );
        // Slicing at the foot of a wall must return the wall's footprint, not nothing.
        let found = crossing(&mesh, 0, 0.0).expect("the face rises out of the plane");
        assert_eq!(found.start, Vec2::new(0.0, 0.0));
        assert_eq!(found.end, Vec2::new(1.0, 0.0));
    }

    #[test]
    fn a_vertex_halfway_up_lands_the_contour_exactly_on_it() {
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 1.0),
                Vec3::new(0.0, 1.0, 2.0),
            ],
            vec![[0, 1, 2]],
        );
        let found = crossing(&mesh, 0, 1.0).expect("crosses");
        // The middle vertex sits on the plane, so one end of the segment is that vertex
        // and no zero-length or duplicated segment appears.
        assert_eq!(found.end, Vec2::new(1.0, 0.0));
        assert_ne!(found.start, found.end);
    }

    #[test]
    fn a_face_lying_in_the_plane_produces_no_crossing() {
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 2.0),
                Vec3::new(1.0, 0.0, 2.0),
                Vec3::new(0.0, 1.0, 2.0),
            ],
            vec![[0, 1, 2]],
        );
        assert!(crossing(&mesh, 0, 2.0).is_none());
    }

    #[test]
    fn a_plane_above_the_face_produces_no_crossing() {
        assert!(crossing(&wall(), 0, 5.0).is_none());
    }

    #[test]
    fn an_out_of_range_face_produces_no_crossing() {
        assert!(crossing(&wall(), 9, 0.5).is_none());
    }

    #[test]
    fn the_crossing_point_sits_where_the_edge_meets_the_plane() {
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(4.0, 0.0, 4.0),
                Vec3::new(0.0, 4.0, 4.0),
            ],
            vec![[0, 1, 2]],
        );
        let found = crossing(&mesh, 0, 1.0).expect("crosses");
        // A quarter of the way up an edge that rises 4 mm over 4 mm of run.
        for point in [found.start, found.end] {
            assert!((point.x + point.y - 1.0).abs() < 1e-6);
        }
    }
}

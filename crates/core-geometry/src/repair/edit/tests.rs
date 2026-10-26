use proptest::prelude::*;

use super::*;
use crate::{Vec3, ball, cuboid, diagnose, faults, orient_outward, signed_volume};

/// `mesh` with the faces lying flat at height `z` taken off: a square hole there.
fn topless(mut mesh: Mesh, z: f32) -> Mesh {
    let vertices = mesh.vertices.clone();
    mesh.faces.retain(|face| {
        !face
            .iter()
            .all(|&corner| (vertices[corner as usize].z - z).abs() < 1e-6)
    });
    mesh
}

fn region(faces: &[usize]) -> Region {
    let mut region = Region::default();
    for face in faces {
        region.set(*face, true);
    }
    region
}

/// The two open edges of the unit cube's top hole that do not touch, as their vertices.
fn opposite_rim_edges(mesh: &Mesh) -> ([u32; 2], [u32; 2]) {
    let open = faults(mesh).open_edges;
    let first = open[0];
    let apart = open
        .iter()
        .find(|edge| !edge.iter().any(|corner| first.contains(corner)))
        .expect("a square rim has an edge across from each");
    (first, *apart)
}

#[test]
fn a_face_turned_the_wrong_way_by_hand_is_found_by_orientation() {
    let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
    assert_eq!(flip_faces(&mut mesh, &region(&[3])), 1);
    assert_eq!(
        orient_outward(&mut mesh).flipped_faces,
        1,
        "the one turned is turned back"
    );
}

#[test]
fn dropping_two_faces_of_a_cube_opens_a_hole_of_four_edges() {
    let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
    assert_eq!(
        remove_faces(&mut mesh, &region(&[0, 1, 40])),
        2,
        "face 40 is not there"
    );
    let found = diagnose(&mesh);
    assert_eq!(found.boundary_edges, 4);
    assert_eq!(found.vertices, 8, "the vertices stay");
}

#[test]
fn bridge_closes_a_square_hole_into_the_unit_cube() {
    let mut mesh = topless(cuboid(Vec3::ZERO, Vec3::ONE), 1.0);
    let (a, b) = opposite_rim_edges(&mesh);
    assert_eq!(bridge(&mut mesh, a, b), Ok(2));

    assert!(diagnose(&mesh).is_sound(), "no open edge is left");
    let volume = signed_volume(&mesh);
    assert!(
        (volume - 1.0).abs() < 1e-6,
        "wound outward, 1 mm³; got {volume}"
    );
}

#[test]
fn bridging_two_edges_meeting_at_a_corner_takes_one_face() {
    let mut mesh = topless(cuboid(Vec3::ZERO, Vec3::ONE), 1.0);
    let open = faults(&mesh).open_edges;
    let touching = open[1..]
        .iter()
        .find(|edge| edge.iter().any(|corner| open[0].contains(corner)))
        .copied()
        .expect("a square rim has two edges at each corner");
    assert_eq!(bridge(&mut mesh, open[0], touching), Ok(1));
    let found = diagnose(&mesh);
    assert_eq!(
        (found.boundary_edges, found.holes),
        (3, 1),
        "half the square is closed, a triangle of a hole left"
    );
    assert_eq!(
        found.folded_edges(),
        0,
        "the face is wound as those beside it"
    );
}

#[test]
fn bridge_refuses_an_edge_two_faces_share() {
    let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
    let [a, b, _] = mesh.faces[0];
    assert_eq!(
        bridge(&mut mesh, [a, b], [0, 1]),
        Err(BridgeError::NotOpen([a, b]))
    );
}

#[test]
fn bridge_refuses_an_edge_to_itself_either_way_round() {
    let mut mesh = topless(cuboid(Vec3::ZERO, Vec3::ONE), 1.0);
    let [a, b] = faults(&mesh).open_edges[0];
    assert_eq!(
        bridge(&mut mesh, [a, b], [b, a]),
        Err(BridgeError::SameEdge)
    );
}

#[test]
fn bridge_refuses_two_edges_on_one_line() {
    // Two open boxes side by side: the front edges of their rims lie on one line.
    let mut mesh = topless(cuboid(Vec3::ZERO, Vec3::ONE), 1.0);
    mesh.append(&topless(
        cuboid(Vec3::new(2.0, 0.0, 0.0), Vec3::new(3.0, 1.0, 1.0)),
        1.0,
    ));
    let on_front = |edge: &&[u32; 2]| {
        edge.iter().all(|&corner| {
            let at = mesh.vertices[corner as usize];
            at.y.abs() < 1e-6 && (at.z - 1.0).abs() < 1e-6
        })
    };
    let open = faults(&mesh).open_edges;
    let front: Vec<[u32; 2]> = open.iter().filter(on_front).copied().collect();
    let [a, b] = front[..] else {
        panic!("one front rim edge per box, got {}", front.len());
    };
    assert_eq!(bridge(&mut mesh, a, b), Err(BridgeError::Degenerate));
}

proptest! {
    #[test]
    fn flipping_any_faces_twice_gives_the_mesh_back(picked in proptest::collection::vec(0usize..200, 0..40)) {
        let mesh = ball(Vec3::ZERO, 5.0, 8, 12);
        let faces = region(&picked);
        let mut turned = mesh.clone();
        flip_faces(&mut turned, &faces);
        flip_faces(&mut turned, &faces);
        prop_assert_eq!(turned, mesh);
    }
}

use super::*;
use crate::grid::VoxelGrid;
use crate::sdf::{Quantised, TILE_VALUES, encode};
use core_geometry::diagnose;

/// Every edge of `mesh` used as often one way round as the other.
fn balanced(mesh: &Mesh) -> bool {
    let mut uses: HashMap<(u32, u32), i32> = HashMap::new();
    for face in &mesh.faces {
        for corner in 0..3 {
            let (from, to) = (face[corner], face[(corner + 1) % 3]);
            let key = (from.min(to), from.max(to));
            *uses.entry(key).or_default() += if from < to { 1 } else { -1 };
        }
    }
    uses.values().all(|net| *net == 0)
}

/// One stored tile whose lowest layer of X is inside and the rest outside, with no
/// tile stored around it: the surface crosses the edges into the tiles below it, which
/// the field has no values for.
#[test]
fn a_surface_on_a_tiles_lowest_face_is_closed_through_the_tiles_below() {
    let band_mm = 0.4;
    let values: Box<[Quantised]> = (0..TILE_VALUES)
        .map(|index| {
            encode(
                if index % TILE as usize == 0 {
                    -0.1
                } else {
                    0.1
                },
                band_mm,
            )
        })
        .collect();
    let field = Sdf::new(
        VoxelGrid::new(0.2),
        band_mm,
        HashMap::from([(IVec3::ZERO, values)]),
        HashMap::new(),
    );

    let mesh = extract(&field, Cancel::never());
    assert!(!mesh.is_empty(), "the inside layer has a surface around it");
    assert_eq!(diagnose(&mesh).boundary_edges, 0, "the surface is closed");
    assert!(
        balanced(&mesh),
        "every edge is crossed back as often as it is crossed"
    );
}

/// A solid tile with one lattice point quantised to exactly zero: every cell around it
/// crosses the surface at that one point, so their vertices are one point too.
#[test]
fn a_lattice_point_on_the_surface_leaves_no_triangle_without_area() {
    let band_mm = 0.4;
    let touching = in_tile(IVec3::splat(TILE / 2));
    let values: Box<[Quantised]> = (0..TILE_VALUES)
        .map(|index| encode(if index == touching { 0.0 } else { -0.1 }, band_mm))
        .collect();
    let field = Sdf::new(
        VoxelGrid::new(0.2),
        band_mm,
        HashMap::from([(IVec3::ZERO, values)]),
        HashMap::new(),
    );

    let diagnostics = diagnose(&extract(&field, Cancel::never()));
    assert_eq!(diagnostics.degenerate_faces, 0, "got {diagnostics:?}");
    assert_eq!(diagnostics.boundary_edges, 0, "got {diagnostics:?}");
}

#[test]
fn copies_of_a_triangle_add_up_the_way_the_surface_does() {
    let mut mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], Vec::new());
    let mut written = Written::default();
    let (forward, backward) = ([0, 1, 2], [0, 2, 1]);
    for face in [forward, backward, forward, forward] {
        written.add(&mut mesh, face);
    }
    mesh.faces.retain(|face| face[0] != face[1]);

    assert_eq!(
        mesh.faces,
        vec![forward, forward],
        "one copy taken out by its opposite, the two written alike after it kept"
    );
}

/// Corner values for a cell with only the corner at its lowest point inside.
fn one_corner_inside() -> [Scalar; CORNERS] {
    let mut values = [1.0; CORNERS];
    values[0] = -1.0;
    values
}

#[test]
fn a_cell_the_surface_misses_has_no_vertex() {
    assert!(cell_cut(&[1.0; CORNERS], Vec3::ZERO, 1.0).is_none());
    assert!(cell_cut(&[-1.0; CORNERS], Vec3::ZERO, 1.0).is_none());
}

/// Three edges leave the inside corner, each cut at its middle, so the vertex sits a
/// sixth of the cell along each axis away from that corner.
#[test]
fn a_cell_with_one_corner_inside_puts_its_vertex_near_that_corner() {
    let cut = cell_cut(&one_corner_inside(), Vec3::ZERO, 1.0).expect("the surface crosses it");
    let vertex = cut.at;
    assert!(
        (vertex - Vec3::splat(1.0 / 6.0)).length() < 1e-5,
        "three cuts at 0.5 on the axes average to a sixth on each, got {vertex}"
    );
}

/// The three edges leaving the lowest corner are cut, so this cell claims three quads;
/// a cell the surface misses claims none.
#[test]
fn a_cell_claims_one_quad_for_each_of_its_own_edges_the_surface_cuts() {
    let mut quads = Vec::new();
    quads_at(&one_corner_inside(), IVec3::ZERO, &mut quads);
    assert_eq!(quads.len(), 3);

    quads.clear();
    quads_at(&[1.0; CORNERS], IVec3::ZERO, &mut quads);
    assert!(quads.is_empty());
}

/// A run of cells lying on one plane is one facet; the two sides of a wall thinner
/// than the slack are not, however near that plane they both lie.
#[test]
fn a_flat_run_is_one_facet_and_two_opposed_faces_are_not() {
    let flat: Vec<(IVec3, Cut)> = (0..4)
        .map(|step| {
            (
                IVec3::new(step, 0, 0),
                Cut {
                    at: Vec3::new(step as Scalar, 0.0, 0.0),
                    normal: Vec3::Z,
                },
            )
        })
        .collect();
    assert!(one_facet(&flat, 0.05));

    let mut opposed = flat.clone();
    opposed[3].1.normal = -Vec3::Z;
    assert!(!one_facet(&opposed, 0.05));
}

/// A cell standing further off the shared plane than the slack keeps its own vertex.
#[test]
fn a_cell_off_the_plane_by_more_than_the_slack_is_not_clustered() {
    let mut bent: Vec<(IVec3, Cut)> = (0..4)
        .map(|step| {
            (
                IVec3::new(step, 0, 0),
                Cut {
                    at: Vec3::new(step as Scalar, 0.0, 0.0),
                    normal: Vec3::Z,
                },
            )
        })
        .collect();
    bent[0].1.at.z = 1.0;
    assert!(!one_facet(&bent, 0.05));
}

/// Every quad names four different cells, or the surface would come out with a
/// triangle of no area in it.
#[test]
fn a_quad_names_four_different_cells() {
    let mut quads = Vec::new();
    quads_at(&one_corner_inside(), IVec3::new(3, -4, 5), &mut quads);
    for quad in quads {
        let mut seen = quad.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 4, "a quad named the same cell twice");
    }
}

use std::sync::Arc;

use core_geometry::{Mesh, MeshDiagnostics, Orientation, Transform, Vec3};

use super::marks::Mark;
use super::*;
use crate::scene::{ImportSummary, Imported};

fn summary(unbalanced_edges: usize, boundary_edges: usize) -> ImportSummary {
    let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
    ImportSummary {
        vertices_merged: 0,
        faces_removed: 0,
        orientation: Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: unbalanced_edges == boundary_edges,
        },
        diagnostics: MeshDiagnostics {
            boundary_edges,
            unbalanced_edges,
            ..core_geometry::diagnose(&mesh)
        },
    }
}

/// A plate of `count` models, none of them selected.
fn plate_of(count: usize) -> (Scene, Vec<ObjectId>) {
    let mesh = Arc::new(Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::Y],
        vec![[0, 1, 2]],
    ));
    let mut scene = Scene::default();
    let ids = (0..count)
        .map(|at| {
            scene.insert(Imported::new(
                format!("model-{at}.stl"),
                Arc::clone(&mesh),
                Transform::default(),
                summary(0, 0),
            ))
        })
        .collect();
    scene.clear_selection();
    (scene, ids)
}

#[test]
fn a_rows_menu_moves_everything_picked_when_that_row_is_one_of_them() {
    let (mut scene, ids) = plate_of(3);
    scene.select_many(&ids[..2]);

    assert_eq!(with_the_selection(&scene, ids[1]), ids[..2].to_vec());
}

#[test]
fn a_rows_menu_moves_that_row_alone_when_it_is_not_picked() {
    let (mut scene, ids) = plate_of(3);
    scene.select_many(&ids[..2]);

    assert_eq!(with_the_selection(&scene, ids[2]), vec![ids[2]]);
}

#[test]
fn a_clean_import_says_nothing() {
    assert!(trouble(&summary(0, 0)).is_none());
}

#[test]
fn merging_the_vertices_of_an_stl_is_not_trouble() {
    let merged = ImportSummary {
        vertices_merged: 28,
        ..summary(0, 0)
    };
    assert!(
        trouble(&merged).is_none(),
        "every STL merges vertices, so a mark for it would stand on every row"
    );
}

#[test]
fn a_mesh_with_a_hole_is_named_broken() {
    let (mark, lines) = trouble(&summary(4, 4)).expect("a torn mesh has something to say");
    assert_eq!(mark, Mark::Broken);
    assert_eq!(lines.len(), 1, "the open edges, and nothing more");
}

#[test]
fn faces_folded_onto_each_other_get_the_quiet_mark() {
    let (mark, lines) = trouble(&summary(4, 0)).expect("a fold is worth a line");
    assert_eq!(mark, Mark::Repaired);
    assert_eq!(lines.len(), 1);
}

#[test]
fn a_mesh_repair_only_tidied_gets_the_quiet_mark() {
    let tidied = ImportSummary {
        faces_removed: 2,
        ..summary(0, 0)
    };
    let (mark, lines) = trouble(&tidied).expect("a dropped face is worth a line");
    assert_eq!(mark, Mark::Repaired);
    assert_eq!(lines.len(), 1);
}

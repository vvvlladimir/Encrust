use std::sync::Arc;

use core_geometry::{Mesh, Scalar, Transform, Vec3, transform_mesh};
use printer_profiles::Compensation;

use crate::plate::Model;

/// Bakes every model and the supports under it into one mesh in plate coordinates, ready
/// to slice. `None` when nothing handed over has any geometry.
///
/// The slicer takes a single mesh, and two models that overlap are one solid on the
/// plate, so placement is applied here rather than layer by layer. A hollowed model goes
/// in as its shell — the outer surface with the cavity wound the other way — because the
/// rasteriser's positive winding rule is what takes the cavity out; see
/// `docs/decisions/0059`. Supports are already in plate coordinates and go in as they are.
///
/// Each model is scaled about its own footprint and about the plate, not about the plate's
/// contents: a part shrinks towards itself and is held at the plate while it prints, so a
/// correction must not move its neighbours; see `docs/design/compensation.md`.
pub fn bake(models: &[Model], compensation: &Compensation) -> Option<Mesh> {
    let mut merged = Mesh::default();
    for model in models {
        let part = compensated(placed(model), compensation);
        // The first part becomes the bake rather than being copied into it.
        if merged.vertices.is_empty() {
            merged = part;
        } else {
            append(&mut merged, &part);
        }
    }
    (!merged.is_empty()).then_some(merged)
}

/// One model with its cuts and supports, in plate coordinates.
fn placed(model: &Model) -> Mesh {
    let mut part = if model.transform == Transform::default() {
        model.mesh.as_ref().clone()
    } else {
        transform_mesh(&model.mesh, model.transform)
    };
    if let Some(cuts) = &model.cuts {
        append(&mut part, &transform_mesh(cuts, model.transform));
    }
    for supports in &model.supports {
        append(&mut part, supports);
    }
    part
}

/// Every mesh on the plate and where it stands, without copying any of them.
///
/// The thumbnail is rendered from this rather than from [`bake`], because a render reads
/// each vertex once wherever it lives and merging would copy the whole plate for nothing.
/// Support meshes are already in plate coordinates.
pub fn parts(models: &[Model]) -> Vec<(Arc<Mesh>, Transform)> {
    let mut parts = Vec::new();
    for model in models {
        parts.push((Arc::clone(&model.mesh), model.transform));
        for supports in &model.supports {
            parts.push((Arc::clone(supports), Transform::default()));
        }
    }
    parts
}

/// `part` grown or shrunk to come out at the size it was modelled at, held where it
/// stands: the scale is taken about the middle of its footprint and about the plate.
fn compensated(part: Mesh, compensation: &Compensation) -> Mesh {
    let Some(bounds) = part.aabb().filter(|_| !compensation.scales_nothing()) else {
        return part;
    };
    let (scale, translation) = compensation.placement(
        Scalar::midpoint(bounds.mins.x, bounds.maxs.x),
        Scalar::midpoint(bounds.mins.y, bounds.maxs.y),
    );
    let matrix = Transform {
        translation: Vec3::from_array(translation),
        scale: Vec3::from_array(scale),
        ..Transform::default()
    }
    .to_matrix();
    // In place: a shrink factor is never negative, so no face turns inside out.
    let mut part = part;
    for vertex in &mut part.vertices {
        *vertex = matrix.transform_point3(*vertex);
    }
    part
}

fn append(merged: &mut Mesh, mesh: &Mesh) {
    let offset = merged.vertices.len() as u32;
    merged.vertices.extend_from_slice(&mesh.vertices);
    merged.faces.extend(
        mesh.faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
}

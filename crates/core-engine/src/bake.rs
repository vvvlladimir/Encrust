use std::sync::Arc;

use core_geometry::{Mesh, Scalar, Transform, Vec3, transform_mesh};
use printer_profiles::Compensation;

use crate::plate::Model;

/// A plate baked into one mesh, with the height the material in it reaches.
#[derive(Debug, Clone, PartialEq)]
pub struct Baked {
    pub mesh: Mesh,
    /// Top of what prints, plate millimetres: the models and the supports, leaving out
    /// the bodies that only subtract.
    ///
    /// A cut reaches past the surface it pierces so that no film is left over its mouth
    /// (ADR 0071), so a hole drilled near the top of a model stands above everything that
    /// prints. Cutting to the baked mesh's own box would plan empty layers over the plate.
    pub ceiling_mm: Scalar,
    /// Bottom of what prints, plate millimetres, by the same rule as `ceiling_mm`: a hole
    /// drilled into the underside of a model reaches below it and is no part of the stack,
    /// so only material under the plate is material lost.
    pub floor_mm: Scalar,
}

impl Baked {
    /// A mesh that carries nothing but material, so the stack spans its own box.
    pub fn of(mesh: Mesh) -> Option<Self> {
        let bounds = mesh.aabb()?;
        Some(Self {
            mesh,
            ceiling_mm: bounds.maxs.z,
            floor_mm: bounds.mins.z,
        })
    }
}

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
pub fn bake(models: &[Model], compensation: &Compensation) -> Option<Baked> {
    let mut merged = Mesh::default();
    let mut ceiling_mm = Scalar::NEG_INFINITY;
    let mut floor_mm = Scalar::INFINITY;
    for model in models {
        let part = compensated(placed(model), compensation);
        if let Some(bounds) = part.material.aabb() {
            ceiling_mm = ceiling_mm.max(bounds.maxs.z);
            floor_mm = floor_mm.min(bounds.mins.z);
        }
        // The first part becomes the bake rather than being copied into it.
        if merged.vertices.is_empty() {
            merged = part.material;
        } else {
            append(&mut merged, &part.material);
        }
        append(&mut merged, &part.cuts);
    }
    (!merged.is_empty()).then_some(Baked {
        mesh: merged,
        ceiling_mm,
        floor_mm,
    })
}

/// One model in plate coordinates, with what prints kept apart from what only subtracts.
struct Placed {
    material: Mesh,
    cuts: Mesh,
}

fn placed(model: &Model) -> Placed {
    let mut material = if model.transform == Transform::default() {
        model.mesh.as_ref().clone()
    } else {
        transform_mesh(&model.mesh, model.transform)
    };
    for supports in &model.supports {
        append(&mut material, supports);
    }
    let cuts = match &model.cuts {
        Some(cuts) => transform_mesh(cuts, model.transform),
        None => Mesh::default(),
    };
    Placed { material, cuts }
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
///
/// The footprint is the material's, so a cut riding out past the surface cannot move the
/// part it was drilled in.
fn compensated(part: Placed, compensation: &Compensation) -> Placed {
    let Some(bounds) = part
        .material
        .aabb()
        .filter(|_| !compensation.scales_nothing())
    else {
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
    for vertex in part
        .material
        .vertices
        .iter_mut()
        .chain(&mut part.cuts.vertices)
    {
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

use std::sync::Arc;

use core_geometry::{Aabb, Mat4, Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_slicer::{Repeated, Solid};
use printer_profiles::Compensation;

use crate::plate::Model;

/// A plate baked for cutting, with the height the material in it reaches.
#[derive(Debug, Clone, PartialEq)]
pub struct Baked {
    /// Everything on the plate in plate coordinates, each group of copies held once.
    pub solid: Solid,
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
            solid: mesh.into(),
            ceiling_mm: bounds.maxs.z,
            floor_mm: bounds.mins.z,
        })
    }
}

/// Bakes every model and the supports under it in plate coordinates, ready to slice, or
/// `None` when nothing has geometry; see ADRs 0059, 0258 and `docs/design/compensation.md`.
pub fn bake(models: &[Model], compensation: &Compensation) -> Option<Baked> {
    let mut solid = Solid::default();
    let mut extent = Extent::default();
    for copies in copies_of(models) {
        match copies.as_slice() {
            [model] => {
                let part = compensated(placed(model), compensation);
                extent.take(&part.material);
                // The first part becomes the bake rather than being copied into it.
                if solid.mesh.vertices.is_empty() {
                    solid.mesh = part.material;
                } else {
                    append(&mut solid.mesh, &part.material);
                }
                append(&mut solid.mesh, &part.cuts);
            }
            [first, rest @ ..] => repeat(first, rest, compensation, &mut solid, &mut extent),
            [] => {}
        }
    }
    (!solid.is_empty()).then_some(Baked {
        solid,
        ceiling_mm: extent.ceiling_mm,
        floor_mm: extent.floor_mm,
    })
}

/// How high and how low the material on the plate reaches, plate millimetres.
struct Extent {
    ceiling_mm: Scalar,
    floor_mm: Scalar,
}

impl Default for Extent {
    fn default() -> Self {
        Self {
            ceiling_mm: Scalar::NEG_INFINITY,
            floor_mm: Scalar::INFINITY,
        }
    }
}

impl Extent {
    fn take(&mut self, material: &Mesh) {
        if let Some(bounds) = material.aabb() {
            self.ceiling_mm = self.ceiling_mm.max(bounds.maxs.z);
            self.floor_mm = self.floor_mm.min(bounds.mins.z);
        }
    }
}

/// The models in groups of copies, in the order each group first appears.
fn copies_of(models: &[Model]) -> Vec<Vec<&Model>> {
    let mut groups: Vec<Vec<&Model>> = Vec::new();
    for model in models {
        match groups.iter_mut().find(|group| is_copy(group[0], model)) {
            Some(group) => group.push(model),
            None => groups.push(vec![model]),
        }
    }
    groups
}

/// One mesh and one set of cuts, standing the same way at the same height: `copy` is
/// `model` moved along the plate and in no other way.
fn is_copy(model: &Model, copy: &Model) -> bool {
    let same_cuts = match (&model.cuts, &copy.cuts) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    };
    let (a, b) = (model.transform, copy.transform);
    Arc::ptr_eq(&model.mesh, &copy.mesh)
        && same_cuts
        && (a.rotation, a.scale, a.stretch) == (b.rotation, b.scale, b.stretch)
        // Exactly one height: a hair apart, the planes would cut the copies differently.
        && a.translation.z.total_cmp(&b.translation.z).is_eq()
}

/// A group of copies put on the plate as one body and the offsets it is laid down again
/// at. The supports stay with their own model, since each model's were built for it.
fn repeat(
    first: &Model,
    rest: &[&Model],
    compensation: &Compensation,
    solid: &mut Solid,
    extent: &mut Extent,
) {
    let bare = Model {
        supports: Vec::new(),
        ..first.clone()
    };
    let mut body = placed(&bare);
    let Some(bounds) = body.material.aabb() else {
        return;
    };
    let correction_of = |model: &Model| {
        let moved = model.transform.translation - first.transform.translation;
        correction(compensation, &footprint(model, &bounds, moved))
    };

    let at_first = correction_of(first);
    let origin =
        |matrix: Option<Mat4>, from: Vec3| matrix.map_or(from, |m| m.transform_point3(from));
    let offsets_mm = rest
        .iter()
        .map(|model| {
            let moved = model.transform.translation - first.transform.translation;
            (origin(correction_of(model), moved) - origin(at_first, Vec3::ZERO)).truncate()
        })
        .collect();

    corrected(&mut body.material, at_first);
    corrected(&mut body.cuts, at_first);
    extent.take(&body.material);
    append(&mut body.material, &body.cuts);
    solid.repeated.push(Repeated {
        mesh: body.material,
        offsets_mm,
    });
    for model in std::iter::once(first).chain(rest.iter().copied()) {
        add_supports(model, correction_of(model), solid, extent);
    }
}

/// `model`'s supports into the mesh, shrunk with the model they stand under.
fn add_supports(model: &Model, matrix: Option<Mat4>, solid: &mut Solid, extent: &mut Extent) {
    for supports in &model.supports {
        let mut supports = supports.as_ref().clone();
        corrected(&mut supports, matrix);
        extent.take(&supports);
        append(&mut solid.mesh, &supports);
    }
}

/// The box a copy's material fills: the body's, moved by `moved`, and its own supports.
fn footprint(model: &Model, body: &Aabb, moved: Vec3) -> Aabb {
    let mut bounds = Aabb::new(body.mins + moved, body.maxs + moved);
    for supports in model.supports.iter().filter_map(|supports| supports.aabb()) {
        bounds.take_point(supports.mins);
        bounds.take_point(supports.maxs);
    }
    bounds
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
/// stands. The footprint is the material's, so a cut riding out past the surface cannot
/// move the part it was drilled in.
fn compensated(mut part: Placed, compensation: &Compensation) -> Placed {
    let matrix = part
        .material
        .aabb()
        .and_then(|bounds| correction(compensation, &bounds));
    corrected(&mut part.material, matrix);
    corrected(&mut part.cuts, matrix);
    part
}

/// The shrinkage taken about the middle of `footprint` and about the plate, or `None`
/// where it scales nothing.
fn correction(compensation: &Compensation, footprint: &Aabb) -> Option<Mat4> {
    if compensation.scales_nothing() {
        return None;
    }
    let (scale, translation) = compensation.placement(
        Scalar::midpoint(footprint.mins.x, footprint.maxs.x),
        Scalar::midpoint(footprint.mins.y, footprint.maxs.y),
    );
    Some(
        Transform {
            translation: Vec3::from_array(translation),
            scale: Vec3::from_array(scale),
            ..Transform::default()
        }
        .to_matrix(),
    )
}

/// In place: a shrink factor is never negative, so no face turns inside out.
fn corrected(mesh: &mut Mesh, matrix: Option<Mat4>) {
    if let Some(matrix) = matrix {
        for vertex in &mut mesh.vertices {
            *vertex = matrix.transform_point3(*vertex);
        }
    }
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

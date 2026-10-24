use core_engine::Model;

use crate::scene::Scene;

/// Every visible model of `plate`, with its cavity, its cuts and its supports, as the
/// engine takes them.
///
/// A hollowed model goes in as its shell: the outer surface with the cavity wound the
/// other way, which is what the rasteriser's positive winding rule takes out; see
/// `docs/decisions/0059`. Nothing is copied — the meshes travel as the `Arc`s the scene
/// holds them in.
pub fn models_of(scene: &Scene, plate: u32) -> Vec<Model> {
    scene
        .printable(plate)
        .map(|object| Model {
            mesh: std::sync::Arc::clone(object.uncut()),
            transform: object.transform,
            cuts: object.hollow.cut_bodies().cloned(),
            drilled: object
                .hollow
                .drilled(object.uncut())
                .map(|drilled| std::sync::Arc::clone(&drilled.mesh)),
            supports: object.supports.meshes().unwrap_or_default().to_vec(),
        })
        .collect()
}

#[cfg(test)]
mod tests;

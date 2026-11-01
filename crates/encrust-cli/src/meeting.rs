//! Which models of a staged plate meet another, said once before the plate is cut
//! (ADR 0390).

use core_engine::Model;
use core_geometry::Transform;
use core_plate::{Body, BodyKind, crossings};

use crate::stage::Part;

/// Notes in each of `parts` the others it meets; `owners` names the part each of `models`,
/// a model, a piece or a plug, is of.
pub fn mark(parts: &mut [Part], models: &[Model], owners: &[usize], talk: bool) {
    let bodies: Vec<Body<'_>> = models
        .iter()
        .zip(owners)
        .flat_map(|(model, &owner)| bodies_of(model, owner))
        .collect();
    let met = crossings(&bodies);
    for &[one, other] in &met {
        let (one_input, other_input) = (parts[one].input.clone(), parts[other].input.clone());
        parts[one].meets.push(other_input);
        parts[other].meets.push(one_input);
    }
    if talk && !met.is_empty() {
        println!("Plate");
        for [one, other] in met {
            let (one, other) = (parts[one].input.display(), parts[other].input.display());
            println!("  meeting        {one} and {other}");
        }
        println!();
    }
}

/// `model` and the supports standing under it, as bodies of `owner`.
fn bodies_of(model: &Model, owner: usize) -> impl Iterator<Item = Body<'_>> {
    let itself = Body {
        owner,
        kind: BodyKind::Model,
        mesh: &model.mesh,
        bvh: None,
        transform: model.transform,
    };
    let supports = model.supports.iter().map(move |mesh| Body {
        owner,
        kind: BodyKind::Supports,
        mesh,
        bvh: None,
        transform: Transform::default(),
    });
    std::iter::once(itself).chain(supports)
}

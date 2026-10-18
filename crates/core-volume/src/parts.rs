//! A model split into the parts it is made of; see `docs/design/parts.md`.

mod neck;
mod pieces;

use core_geometry::{Bvh, DEFAULT_WELD_TOLERANCE, Mesh, Scalar, weld};
use serde::{Deserialize, Serialize};

pub use neck::{PartedOff, part_off};
use pieces::{Body, nest, shells};

/// A scrap this near a part touches it, mm: it was broken off it, not left beside it.
const TOUCHING_MM: Scalar = 0.2;

/// What is too small to be a part of its own when a model is split.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartSettings {
    /// A piece holding less than this is no part of its own, mm³: it is joined to the
    /// part it touches, or dropped where it touches none.
    pub smallest_mm3: Scalar,
}

impl Default for PartSettings {
    fn default() -> Self {
        Self { smallest_mm3: 2.0 }
    }
}

/// What a split came to: the parts, biggest first, and what was done on the way.
#[derive(Debug, Clone, Default)]
pub struct Parts {
    pub parts: Vec<Mesh>,
    /// Scraps too small to be parts, joined to the part they touch.
    pub joined: usize,
    /// Scraps too small to be parts and touching none, left out.
    pub dropped: usize,
}

/// Splits `mesh` into its parts: bodies that share no edge two faces use, each with the
/// shells inside it, with the scraps too small to be parts joined to the part they touch
/// or dropped.
pub fn parts(mesh: &Mesh, settings: &PartSettings) -> Parts {
    let welded = weld(mesh, DEFAULT_WELD_TOLERANCE).mesh;
    let bodies = nest(shells(&welded));
    let mut done = Parts::default();
    let (mut kept, scraps): (Vec<Body>, Vec<Body>) = bodies
        .into_iter()
        .partition(|body| body.size_mm3 >= settings.smallest_mm3);
    if kept.is_empty() {
        // Nothing is big enough to stand for the model: every piece is kept as it is.
        kept = scraps;
        return finished(kept, done);
    }
    attach(&mut kept, scraps, TOUCHING_MM, &mut done);
    finished(kept, done)
}

/// Joins each of `scraps` to the nearest part of `parts` within `reach_mm`, and drops
/// the rest.
fn attach(parts: &mut [Body], scraps: Vec<Body>, reach_mm: Scalar, done: &mut Parts) {
    if scraps.is_empty() {
        return;
    }
    let hierarchies: Vec<Bvh> = parts.iter().map(|part| Bvh::build(&part.mesh)).collect();
    for scrap in scraps {
        let nearest = parts
            .iter()
            .zip(&hierarchies)
            .enumerate()
            .map(|(index, (part, bvh))| (index, gap(&scrap.mesh, &part.mesh, bvh)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match nearest {
            Some((index, gap)) if gap <= reach_mm => {
                parts[index].mesh.append(&scrap.mesh);
                done.joined += 1;
            }
            _ => done.dropped += 1,
        }
    }
}

/// How close `scrap` comes to `part`, mm, from its nearest vertex.
fn gap(scrap: &Mesh, part: &Mesh, bvh: &Bvh) -> Scalar {
    scrap
        .vertices
        .iter()
        .filter_map(|vertex| bvh.closest(part, *vertex))
        .map(|closest| closest.distance)
        .fold(Scalar::MAX, Scalar::min)
}

fn finished(mut bodies: Vec<Body>, mut done: Parts) -> Parts {
    for body in &mut bodies {
        *body = Body::of(std::mem::take(&mut body.mesh));
    }
    bodies.sort_by(|a, b| b.size_mm3.total_cmp(&a.size_mm3));
    done.parts = bodies.into_iter().map(|body| body.mesh).collect();
    done
}

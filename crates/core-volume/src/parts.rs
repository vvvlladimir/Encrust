//! A model split into the parts it is made of; see `docs/design/parts.md`.

mod necks;
mod pieces;

use core_geometry::{Bvh, DEFAULT_WELD_TOLERANCE, Mesh, Scalar, weld};
use serde::{Deserialize, Serialize};

use crate::cancel::Cancel;
use crate::error::VolumeError;
use pieces::{Body, nest, shells};

/// A scrap this near a part touches it, mm: it was broken off it, not left beside it.
const TOUCHING_MM: Scalar = 0.2;

/// What a model is split by, and what is too small to be a part of its own.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartSettings {
    /// Whether a part is also parted where two thicker bodies are joined by something
    /// narrower than `neck_mm`.
    pub at_necks: bool,
    pub neck_mm: Scalar,
    /// A piece holding less than this is no part of its own, mm³: it is joined to the
    /// part it touches, or dropped where it touches none.
    pub smallest_mm3: Scalar,
}

impl Default for PartSettings {
    fn default() -> Self {
        Self {
            at_necks: false,
            neck_mm: 1.0,
            smallest_mm3: 2.0,
        }
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
    /// Narrow joins the parts were parted at.
    pub necks: usize,
    /// Bodies that had narrow joins but could not be cut exactly, and stay whole.
    pub unparted: usize,
}

/// Splits `mesh` into its parts: bodies that share no edge two faces use, each with the
/// shells inside it, parted at narrow joins if `settings` asks, with the scraps too small
/// to be parts joined to the part they touch or dropped.
///
/// `cancel` is asked while narrow joins are looked for; a run given up on is
/// [`VolumeError::Cancelled`].
pub fn parts(
    mesh: &Mesh,
    settings: &PartSettings,
    cancel: Cancel<'_>,
) -> Result<Parts, VolumeError> {
    let welded = weld(mesh, DEFAULT_WELD_TOLERANCE).mesh;
    let bodies = nest(shells(&welded));
    let mut done = Parts::default();
    let (mut kept, scraps): (Vec<Body>, Vec<Body>) = bodies
        .into_iter()
        .partition(|body| body.size_mm3 >= settings.smallest_mm3);
    if kept.is_empty() {
        // Nothing is big enough to stand for the model: every piece is kept as it is.
        kept = scraps;
        return Ok(finished(kept, done));
    }
    if settings.at_necks {
        kept = parted_at_necks(kept, settings, cancel, &mut done)?;
    }
    attach(&mut kept, scraps, TOUCHING_MM, &mut done);
    Ok(finished(kept, done))
}

/// Every body of `kept` parted at its narrow joins, the scraps of each cut joined to the
/// part they touch, and a body the cut cannot read left whole.
fn parted_at_necks(
    kept: Vec<Body>,
    settings: &PartSettings,
    cancel: Cancel<'_>,
    done: &mut Parts,
) -> Result<Vec<Body>, VolumeError> {
    let mut parted = Vec::with_capacity(kept.len());
    for body in kept {
        let pieces =
            match necks::parted(&body.mesh, settings.neck_mm, settings.smallest_mm3, cancel) {
                Ok(pieces) => pieces,
                Err(VolumeError::NotSolid(_)) => {
                    done.unparted += 1;
                    parted.push(body);
                    continue;
                }
                Err(error) => return Err(error),
            };
        if pieces.len() == 1 {
            parted.push(body);
            continue;
        }
        done.necks += pieces.len() - 1;
        let bodies: Vec<Body> = pieces
            .iter()
            .flat_map(|piece| nest(shells(piece)))
            .collect();
        let (mut whole, scraps): (Vec<Body>, Vec<Body>) = bodies
            .into_iter()
            .partition(|body| body.size_mm3 >= settings.smallest_mm3);
        // A cut's scraps are the body's own material, so each goes back to the nearest part
        // however far it is; it is not counted, being no scrap of the model's.
        attach(&mut whole, scraps, Scalar::MAX, &mut Parts::default());
        parted.extend(whole);
    }
    Ok(parted)
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

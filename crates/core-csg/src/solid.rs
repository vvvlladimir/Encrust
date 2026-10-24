use std::panic::{self, AssertUnwindSafe};

use boolmesh::prelude::{Manifold, OpType, compute_boolean};
use core_geometry::{Aabb, Components, Mesh, Vec3};
use rayon::prelude::*;

use crate::error::CsgError;
use crate::seams::{Unzipped, unzipped};

/// What the engine answers when a boolean leaves nothing behind; it says so in its own
/// tests, which is the only place the case is stated.
const EMPTY: &str = "empty pos matrix";

/// A closed mesh readied for booleans, or the empty solid.
///
/// Combining never changes either side; each operation answers a new solid, so one
/// cavity can be cut by several lattices in turn.
#[derive(Clone)]
pub struct Solid {
    inner: Option<Manifold>,
}

impl std::fmt::Debug for Solid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Solid")
            .field("faces", &self.inner.as_ref().map_or(0, |inner| inner.nf))
            .finish()
    }
}

impl Solid {
    /// The solid nothing is in.
    pub fn empty() -> Self {
        Self { inner: None }
    }

    /// `mesh` read as a solid. It may be several closed shells, touching along an edge or
    /// at a vertex but crossing none; vertices on the same exact point are taken as one.
    pub fn new(mesh: &Mesh) -> Result<Self, CsgError> {
        let Unzipped { positions, indices } = unzipped(mesh)?;
        // Welding can leave no face at all, which the engine would refuse as not solid.
        if indices.is_empty() {
            return Ok(Self::empty());
        }
        guarded(|| Manifold::new(&positions, &indices))
            .map(|inner| Self { inner: Some(inner) })
            .map_err(CsgError::NotSolid)
    }

    /// Whether the solid holds nothing.
    pub fn is_empty(&self) -> bool {
        self.inner.is_none()
    }

    /// Everything either solid holds.
    pub fn union(&self, other: &Self) -> Result<Self, CsgError> {
        match (&self.inner, &other.inner) {
            (None, _) => Ok(other.clone()),
            (_, None) => Ok(self.clone()),
            (Some(left), Some(right)) => combined(left, right, OpType::Add),
        }
    }

    /// This solid with `other` taken out of it.
    pub fn difference(&self, other: &Self) -> Result<Self, CsgError> {
        match (&self.inner, &other.inner) {
            (None, _) => Ok(Self::empty()),
            (_, None) => Ok(self.clone()),
            (Some(left), Some(right)) => combined(left, right, OpType::Subtract),
        }
    }

    /// Only what both solids hold.
    pub fn intersection(&self, other: &Self) -> Result<Self, CsgError> {
        match (&self.inner, &other.inner) {
            (Some(left), Some(right)) => combined(left, right, OpType::Intersect),
            _ => Ok(Self::empty()),
        }
    }

    /// Everything any of `solids` holds. Solids whose boxes meet are unioned in groups on
    /// every core; the groups are then set side by side with no boolean, since nothing of one
    /// reaches another (ADR 0316).
    pub fn union_all(solids: &[Solid]) -> Result<Self, CsgError> {
        let parts: Vec<&Manifold> = solids
            .iter()
            .filter_map(|solid| solid.inner.as_ref())
            .collect();
        let merged = meeting(&parts)
            .into_par_iter()
            .map(|group| merged(group.into_iter().map(|at| parts[at]).collect()))
            .collect::<Result<Vec<_>, _>>()?;
        composed(
            &merged
                .iter()
                .filter_map(|solid| solid.inner.as_ref())
                .collect::<Vec<_>>(),
        )
    }

    /// The solid's surface as a mesh, wound outward.
    pub fn mesh(&self) -> Mesh {
        let Some(inner) = &self.inner else {
            return Mesh::default();
        };
        let vertices = inner
            .ps
            .iter()
            .map(|point| Vec3::new(point.x as f32, point.y as f32, point.z as f32))
            .collect();
        let faces = inner
            .hs
            .as_chunks::<3>()
            .0
            .iter()
            .map(|corners| corners.each_ref().map(|half| half.tail))
            .collect();
        Mesh::new(vertices, faces)
    }

    /// The box the solid fills, or `None` for the empty solid.
    pub fn aabb(&self) -> Option<Aabb> {
        let bounds = &self.inner.as_ref()?.bb;
        let (low, high) = (bounds.min, bounds.max);
        Some(Aabb::new(
            Vec3::new(low.x as f32, low.y as f32, low.z as f32),
            Vec3::new(high.x as f32, high.y as f32, high.z as f32),
        ))
    }
}

/// The parts as one solid, side by side.
fn composed(parts: &[&Manifold]) -> Result<Solid, CsgError> {
    match parts {
        [] => Ok(Solid::empty()),
        [only] => Ok(Solid {
            inner: Some((*only).clone()),
        }),
        _ => guarded(|| Manifold::compose(parts))
            .map(|inner| Solid { inner: Some(inner) })
            .map_err(CsgError::Failed),
    }
}

/// Whether two boxes share a point; touching counts, as the solids may then meet on it.
fn meet(a: &Manifold, b: &Manifold) -> bool {
    a.bb.min.cmple(b.bb.max).all() && b.bb.min.cmple(a.bb.max).all()
}

/// The parts in groups whose boxes chain into each other, each group in the parts' order.
fn meeting(parts: &[&Manifold]) -> Vec<Vec<usize>> {
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by(|&a, &b| parts[a].bb.min.x.total_cmp(&parts[b].bb.min.x));
    let mut groups = Components::new(parts.len());
    // Swept along x: a part can only meet one whose span in x has not ended before it starts.
    let mut open: Vec<usize> = Vec::new();
    for &at in &order {
        open.retain(|&other| parts[other].bb.max.x >= parts[at].bb.min.x);
        for &other in &open {
            if meet(parts[at], parts[other]) {
                groups.union(at as u32, other as u32);
            }
        }
        open.push(at);
    }
    let (labels, count) = groups.labels();
    let mut out = vec![Vec::new(); count];
    for (at, label) in labels.into_iter().enumerate() {
        out[label as usize].push(at);
    }
    out
}

/// One group's union down a tree, in the parts' order. A part with more faces than all the
/// others together is taken in last, so a dense model under a few small parts goes through
/// the engine once rather than once a level of the tree.
fn merged(mut group: Vec<&Manifold>) -> Result<Solid, CsgError> {
    let total: usize = group.iter().map(|part| part.nf).sum();
    let largest = (0..group.len()).max_by_key(|&at| group[at].nf);
    match largest {
        Some(at) if 2 * group[at].nf > total && group.len() > 1 => {
            let largest = Solid {
                inner: Some(group.remove(at).clone()),
            };
            largest.union(&paired(&group)?)
        }
        _ => paired(&group),
    }
}

/// The parts unioned two by two down a tree, side by side where their boxes do not meet.
fn paired(parts: &[&Manifold]) -> Result<Solid, CsgError> {
    match parts {
        [] => Ok(Solid::empty()),
        [only] => Ok(Solid {
            inner: Some((*only).clone()),
        }),
        _ => {
            let (left, right) = parts.split_at(parts.len() / 2);
            let (left, right) = rayon::join(|| paired(left), || paired(right));
            let (left, right) = (left?, right?);
            match (&left.inner, &right.inner) {
                (Some(a), Some(b)) if !meet(a, b) => composed(&[a, b]),
                _ => left.union(&right),
            }
        }
    }
}

fn combined(left: &Manifold, right: &Manifold, op: OpType) -> Result<Solid, CsgError> {
    answered(guarded(|| compute_boolean(left, right, op)))
}

/// The engine's answer to a boolean as a solid: nothing left is the empty one.
fn answered(answer: Result<Manifold, String>) -> Result<Solid, CsgError> {
    match answer {
        Ok(inner) => Ok(Solid { inner: Some(inner) }),
        Err(reason) if reason == EMPTY => Ok(Solid::empty()),
        Err(reason) => Err(CsgError::Failed(reason)),
    }
}

/// The engine's answer, or its panic as an error: it indexes past its own tables on some
/// inputs rather than refusing them.
fn guarded<T>(run: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(run))
        .unwrap_or_else(|_| Err("the boolean engine panicked".to_owned()))
}

#[cfg(test)]
mod tests;

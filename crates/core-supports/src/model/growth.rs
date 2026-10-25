use std::sync::Arc;

use super::{Built, ModelSupports};
use crate::{
    Blocked, Clearances, Placed, Profiles, SupportPoint, SupportTree, columns, grow, mesh_groups,
};
use core_geometry::{Bvh, Mesh, Region, Transform};
use printer_profiles::SupportProfile;

/// What the trees of one model are grown from, taken off it so that they can be grown on
/// another thread while the window keeps drawing; see `docs/decisions/0342`.
#[derive(Debug, Clone)]
pub struct Growth {
    pub(super) points: Vec<SupportPoint>,
    pub(super) blocked: Region,
    pub(super) transform: Transform,
    pub(super) table: Vec<SupportProfile>,
    pub(super) revision: u64,
    pub(super) clearances: Arc<Clearances>,
    /// The frozen trees, in the model's own space, meshed beside the grown ones.
    pub(super) frozen: Vec<SupportTree>,
}

impl Growth {
    /// Grows the trees against `surface`, whose hierarchy is `bvh`, keeping out of the
    /// patch blocked on `model`, and meshes them with the frozen ones off the drawing thread.
    pub fn grow(self, model: &Mesh, surface: &Mesh, bvh: &Bvh) -> Grown {
        let keep_out = Blocked::new(model, &self.blocked, self.transform);
        let placed = Placed::new(surface, bvh, self.transform)
            .blocking(keep_out.as_ref())
            .remembering(&self.clearances);
        let (trees, meshes) =
            Profiles::new(&self.table).map_or_else(Default::default, |profiles| {
                let trees = grow(&columns(&self.points, &placed, profiles), &placed, profiles);
                let all = with_frozen(&trees, &self.frozen, self.transform);
                let meshes = mesh_groups(&all, &placed, profiles);
                (trees, meshes.into_iter().map(Arc::new).collect())
            });
        Grown {
            forest: Forest {
                trees,
                transform: self.transform,
                table: self.table,
                blocked: self.blocked,
            },
            revision: self.revision,
            frozen: self.frozen,
            meshes,
        }
    }
}

/// `grown` followed by `frozen` stood on the plate at `transform`: every tree a model meshes.
pub(super) fn with_frozen(
    grown: &[SupportTree],
    frozen: &[SupportTree],
    transform: Transform,
) -> Vec<SupportTree> {
    let matrix = transform.to_matrix();
    let mut trees = grown.to_vec();
    trees.extend(frozen.iter().map(|tree| tree.moved(matrix)));
    trees
}

/// The trees a [`Growth`] grew, to be handed back to the model it was taken from.
#[derive(Debug, Clone)]
pub struct Grown {
    pub(super) forest: Forest,
    pub(super) revision: u64,
    /// The frozen trees `meshes` were built with, which still hold if the model's still match.
    pub(super) frozen: Vec<SupportTree>,
    pub(super) meshes: Vec<Arc<Mesh>>,
}

/// The trees grown from the points, in plate coordinates, and what they were grown against.
#[derive(Debug, Clone)]
pub(super) struct Forest {
    pub(super) trees: Vec<SupportTree>,
    pub(super) transform: Transform,
    pub(super) table: Vec<SupportProfile>,
    pub(super) blocked: Region,
}

impl Forest {
    pub(super) fn grown_for(
        &self,
        transform: Transform,
        table: &[SupportProfile],
        blocked: &Region,
    ) -> bool {
        self.transform == transform && self.table == table && self.blocked == *blocked
    }
}

impl ModelSupports {
    /// What the trees have to be grown from before [`Self::refresh_on`] can build against
    /// `transform` and `table`, or `None` while the trees grown last still hold.
    pub fn growth(&self, transform: Transform, table: &[SupportProfile]) -> Option<Growth> {
        self.needs_growing(transform, table).then(|| Growth {
            points: self.points.clone(),
            blocked: self.blocked.clone(),
            transform,
            table: table.to_vec(),
            revision: self.revision,
            clearances: Arc::clone(&self.clearances),
            frozen: self.frozen.clone(),
        })
    }

    /// Whether [`Self::refresh_on`] against `transform` and `table` would grow the trees
    /// again, which is the part of it too slow for a frame.
    pub fn needs_growing(&self, transform: Transform, table: &[SupportProfile]) -> bool {
        Profiles::new(table).is_some()
            && !self
                .forest
                .as_ref()
                .is_some_and(|forest| forest.grown_for(transform, table, &self.blocked))
    }

    /// Lets go of what was built when the trees must grow again for `transform` and `table`,
    /// so nothing slices it meanwhile; [`Self::drawn`] still shows it. Answers whether they must.
    pub fn outgrown(&mut self, transform: Transform, table: &[SupportProfile]) -> bool {
        let outgrown = self.needs_growing(transform, table);
        if outgrown {
            self.built = None;
        }
        outgrown
    }

    /// What the trees grow from as it last changed; a run grown from another is stale.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Takes on trees grown from a [`Growth`], and answers whether they were still wanted:
    /// nothing they grow from has changed since it was taken. Their meshes hold unless the
    /// frozen trees have been edited meanwhile.
    pub fn take_grown(&mut self, grown: Grown) -> bool {
        if grown.revision != self.revision {
            return false;
        }
        let Grown {
            forest,
            frozen,
            meshes,
            ..
        } = grown;
        self.built = (frozen == self.frozen).then(|| {
            self.drawn.clone_from(&meshes);
            Built {
                trees: with_frozen(&forest.trees, &frozen, forest.transform),
                grown: forest.trees.len(),
                meshes,
                transform: forest.transform,
                table: forest.table.clone(),
                painted: self.painted.clone(),
                blocked: forest.blocked.clone(),
            }
        });
        self.forest = Some(forest);
        true
    }
}

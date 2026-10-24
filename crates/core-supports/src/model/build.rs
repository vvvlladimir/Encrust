use super::ModelSupports;
use super::growth::{Forest, with_frozen};
use crate::{Placed, Profiles, SupportTree, columns, grow, mesh_groups};
use core_geometry::{Bvh, Mesh, Region, Transform};
use printer_profiles::SupportProfile;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(super) struct Built {
    /// Every tree standing under the model: the ones grown from the points first, the
    /// frozen ones after them.
    pub(super) trees: Vec<SupportTree>,
    /// How many of `trees` were grown rather than frozen, which is where the frozen ones
    /// start and what turns a picked tree into an index into `frozen`.
    pub(super) grown: usize,
    /// One mesh per group, in group order, so that each is drawn in a colour of its own.
    pub(super) meshes: Vec<Arc<Mesh>>,
    pub(super) transform: Transform,
    /// The profile of every group as it stood when this was built.
    pub(super) table: Vec<SupportProfile>,
    pub(super) painted: Region,
    pub(super) blocked: Region,
}

impl ModelSupports {
    /// The trees the automatic run grew, in the model's own space: what a project keeps
    /// so that opening it stands them up again rather than deciding where they go anew.
    pub fn grown(&self, transform: Transform) -> Vec<SupportTree> {
        let (Some(built), Some(inverse)) = (self.built.as_ref(), transform.inverse_matrix()) else {
            return Vec::new();
        };
        built.trees[..built.grown]
            .iter()
            .map(|tree| tree.moved(inverse))
            .collect()
    }

    /// Stands `grown` back up beside the frozen trees and meshes them without growing anything, so
    /// opening a project decides nothing; one saved mid-growth grows its trees now.
    pub fn stand(
        &mut self,
        grown: &[SupportTree],
        model: &Mesh,
        surface: &Mesh,
        bvh: &Bvh,
        transform: Transform,
        table: &[SupportProfile],
    ) {
        if grown.is_empty() && !self.points.is_empty() {
            self.refresh_on(model, surface, bvh, transform, table);
            return;
        }
        self.refresh_patches(model, transform);
        let Some(profiles) = Profiles::new(table) else {
            return;
        };
        let matrix = transform.to_matrix();
        let standing = grown.len();
        let stood: Vec<SupportTree> = grown.iter().map(|tree| tree.moved(matrix)).collect();
        let trees = with_frozen(&stood, &self.frozen, transform);
        self.forest = Some(Forest {
            trees: stood,
            transform,
            table: table.to_vec(),
            blocked: self.blocked.clone(),
        });

        let keep_out = self.keep_out(model, transform);
        let placed = Placed::new(surface, bvh, transform)
            .blocking(keep_out.as_ref())
            .remembering(&self.clearances);
        let meshes: Vec<Arc<Mesh>> = mesh_groups(&trees, &placed, profiles)
            .into_iter()
            .map(Arc::new)
            .collect();
        self.drawn.clone_from(&meshes);
        self.built = Some(Built {
            meshes,
            trees,
            grown: standing,
            transform,
            table: table.to_vec(),
            painted: self.painted.clone(),
            blocked: self.blocked.clone(),
        });
    }

    /// Whether the next rebuild against `transform` and `table` would grow the trees again
    /// for the table alone: what a window may put off while a field is still being dragged.
    pub fn retuned(&self, transform: Transform, table: &[SupportProfile]) -> bool {
        self.forest.as_ref().is_some_and(|forest| {
            forest.transform == transform && forest.blocked == self.blocked && forest.table != table
        })
    }

    /// Rebuilds the supports if anything they depend on changed: every point dropped down through
    /// `bvh` (ADR 0036), then tips merged into trunks (ADR 0040); an unrelated edit does neither.
    pub fn refresh(
        &mut self,
        model: &Mesh,
        bvh: &Bvh,
        transform: Transform,
        table: &[SupportProfile],
    ) {
        self.refresh_on(model, model, bvh, transform, table);
    }

    /// [`Self::refresh`] against `surface`, hierarchy `bvh`, rather than `model`, which the
    /// patches are painted on: a hollowed model's drilled shell (ADR 0334).
    pub fn refresh_on(
        &mut self,
        model: &Mesh,
        surface: &Mesh,
        bvh: &Bvh,
        transform: Transform,
        table: &[SupportProfile],
    ) {
        self.refresh_patches(model, transform);
        let Some(profiles) = Profiles::new(table) else {
            return;
        };
        let fresh = self.built.as_ref().is_some_and(|built| {
            built.transform == transform
                && built.table == table
                && built.painted == self.painted
                && built.blocked == self.blocked
        });
        if fresh {
            return;
        }

        let keep_out = self.keep_out(model, transform);
        let placed = Placed::new(surface, bvh, transform)
            .blocking(keep_out.as_ref())
            .remembering(&self.clearances);
        let grown_trees = match &self.forest {
            Some(forest) if forest.grown_for(transform, table, &self.blocked) => {
                forest.trees.clone()
            }
            _ => {
                let trees = grow(&columns(&self.points, &placed, profiles), &placed, profiles);
                self.forest = Some(Forest {
                    trees: trees.clone(),
                    transform,
                    table: table.to_vec(),
                    blocked: self.blocked.clone(),
                });
                trees
            }
        };
        let grown = grown_trees.len();
        // A frozen tree is kept in the model's own space, so it rides the placement the
        // same way a point does; nothing else is asked of it.
        let trees = with_frozen(&grown_trees, &self.frozen, transform);

        let meshes: Vec<Arc<Mesh>> = mesh_groups(&trees, &placed, profiles)
            .into_iter()
            .map(Arc::new)
            .collect();
        self.drawn.clone_from(&meshes);
        self.built = Some(Built {
            meshes,
            trees,
            grown,
            transform,
            table: table.to_vec(),
            painted: self.painted.clone(),
            blocked: self.blocked.clone(),
        });
    }
}

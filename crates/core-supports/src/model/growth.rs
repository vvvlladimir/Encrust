use super::ModelSupports;
use crate::{Blocked, Placed, Profiles, SupportPoint, SupportTree, columns, grow};
use core_geometry::{Bvh, Mesh, Region, Transform};
use printer_profiles::SupportProfile;

/// What the trees of one model are grown from, taken off it so that they can be grown on
/// another thread while the window keeps drawing; see `docs/decisions/0340`.
#[derive(Debug, Clone)]
pub struct Growth {
    pub(super) points: Vec<SupportPoint>,
    pub(super) blocked: Region,
    pub(super) transform: Transform,
    pub(super) table: Vec<SupportProfile>,
    pub(super) revision: u64,
}

impl Growth {
    /// Grows the trees against `surface`, whose hierarchy is `bvh`, keeping out of the
    /// patch blocked on `model`.
    pub fn grow(self, model: &Mesh, surface: &Mesh, bvh: &Bvh) -> Grown {
        let keep_out = Blocked::new(model, &self.blocked, self.transform);
        let placed = Placed::new(surface, bvh, self.transform).blocking(keep_out.as_ref());
        let trees = Profiles::new(&self.table).map_or_else(Vec::new, |profiles| {
            grow(&columns(&self.points, &placed, profiles), &placed, profiles)
        });
        Grown {
            forest: Forest {
                trees,
                transform: self.transform,
                table: self.table,
                blocked: self.blocked,
            },
            revision: self.revision,
        }
    }
}

/// The trees a [`Growth`] grew, to be handed back to the model it was taken from.
#[derive(Debug, Clone)]
pub struct Grown {
    pub(super) forest: Forest,
    pub(super) revision: u64,
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

    /// Takes on trees grown from a [`Growth`], and answers whether they were still wanted:
    /// nothing they grow from has changed since it was taken.
    pub fn take_grown(&mut self, grown: Grown) -> bool {
        if grown.revision != self.revision {
            return false;
        }
        self.forest = Some(grown.forest);
        self.built = None;
        true
    }
}

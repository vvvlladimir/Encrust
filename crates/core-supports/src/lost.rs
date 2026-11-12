use crate::column::{column, has_head};
use crate::group::Profiles;
use crate::placed::Placed;
use crate::{SupportPoint, SupportTree};
use rayon::prelude::*;

/// Why a support point has no support under it (ADR 0429).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unstood {
    /// Its head would run into the model beside its contact.
    HeadBlocked,
    /// It sits on a face looking up, where a trunk from its neck would come down on it.
    FacesUp,
    /// Nothing within reach takes a trunk: no plate, no face it may stand on.
    NoGround,
}

/// A support point no tree carries, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lost {
    /// Index into the points the trees were grown from.
    pub point: usize,
    pub why: Unstood,
}

/// The points of `points` that none of `trees`, grown from them on `placed`, carries.
pub fn lost(
    points: &[SupportPoint],
    trees: &[SupportTree],
    placed: &Placed,
    profiles: Profiles,
) -> Vec<Lost> {
    let mut held = vec![false; points.len()];
    for point in trees.iter().flat_map(SupportTree::points) {
        if let Some(slot) = held.get_mut(point) {
            *slot = true;
        }
    }
    let matrix = placed.transform.to_matrix();
    held.par_iter()
        .zip(points)
        .enumerate()
        .filter(|(_, (held, _))| !**held)
        .map(|(point, (_, support))| {
            let contact = matrix.transform_point3(support.contact);
            let profile = profiles.of(support.group);
            // A column with no head is a small pillar's, which stands where a head had no room.
            let why = match column(placed, point, support.group, contact, profile) {
                Ok(column) if !has_head(column.contact, column.neck) => Unstood::HeadBlocked,
                Ok(_) => Unstood::NoGround,
                Err(why) => why,
            };
            Lost { point, why }
        })
        .collect()
}

#[cfg(test)]
mod tests;

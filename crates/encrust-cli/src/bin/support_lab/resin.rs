//! Where the supports' resin goes: heads, branches, trunks, feet, braces and raft.

use anyhow::{Context, Result};
use core_geometry::{Bvh, Transform};
use core_supports::{Placed, Profiles, resin_by_part};
use serde::{Deserialize, Serialize};

use crate::stand::Supported;

/// The supports' resin by part, millilitres, each body counted whole, and how many trunks
/// stand on a foot of their own.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Resin {
    pub heads_ml: f32,
    pub branches_ml: f32,
    pub trunks_ml: f32,
    pub feet_ml: f32,
    pub braces_ml: f32,
    pub raft_ml: f32,
    /// Trees standing on the plate or the model rather than on another support.
    pub footed: usize,
}

impl Resin {
    pub fn total_ml(&self) -> f32 {
        self.heads_ml
            + self.branches_ml
            + self.trunks_ml
            + self.feet_ml
            + self.braces_ml
            + self.raft_ml
    }

    /// The block's line of it.
    pub fn print(&self) {
        let [heads, branches, trunks, feet, braces, raft] = self.shares();
        println!(
            "- resin by part: heads {heads:.0}%, branches {branches:.0}%, trunks {trunks:.0}%, feet {feet:.0}%, braces {braces:.0}%, raft {raft:.0}% of {:.2} ml; {} trees on a foot of their own",
            self.total_ml(),
            self.footed,
        );
    }

    /// Each part as a share of the total, in percent, in the order the fields stand.
    pub fn shares(&self) -> [f32; 6] {
        let total = self.total_ml().max(1e-6);
        [
            self.heads_ml,
            self.branches_ml,
            self.trunks_ml,
            self.feet_ml,
            self.braces_ml,
            self.raft_ml,
        ]
        .map(|ml| 100.0 * ml / total)
    }
}

pub fn read(supported: &Supported) -> Result<Resin> {
    let bvh = Bvh::build(&supported.model);
    let placed = Placed::new(&supported.model, &bvh, Transform::default());
    let profiles =
        Profiles::new(&supported.profiles).context("a table of profiles is never empty")?;
    let parts = resin_by_part(&supported.trees, &placed, profiles);
    Ok(Resin {
        heads_ml: parts.heads_ml,
        branches_ml: parts.branches_ml,
        trunks_ml: parts.trunks_ml,
        feet_ml: parts.feet_ml,
        braces_ml: parts.braces_ml,
        raft_ml: parts.raft_ml,
        footed: supported
            .trees
            .iter()
            .filter(|tree| !tree.landing().on_support)
            .count(),
    })
}

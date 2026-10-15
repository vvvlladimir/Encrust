//! Reading the stack as it is written: what each layer cures, how hard it pulls on the
//! film as the plate lifts it, and where a print is likely to fail.

mod contact;
mod cups;
mod layer;
mod pull;
mod stack;

pub use contact::{MIN_ISLAND_MM2, island_runs, islands};
pub use cups::{MIN_CUP_MM2, MIN_CUP_MM3};
pub use layer::{Cured, LayerMeasure, Piece, Stretch, cure, erase, measure};
pub use pull::{
    PEEL_LIMIT_N, PULL_N_PER_MM2, PULL_N_PER_MM3, disc_pull_n, equivalent_disc_mm, pull_n,
};
pub use stack::{MIN_THIN_RUN_MM, Measured, NECK_LIMIT_MPA, Peak, Risk, RiskKind, THIN_MM};

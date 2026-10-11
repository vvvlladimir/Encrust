//! Reading the stack as it is written: what each layer cures, how hard it pulls on the
//! film as the plate lifts it, and where a print is likely to fail.

mod contact;
mod cups;
mod layer;
mod stack;

pub use contact::{MIN_ISLAND_MM2, island_runs, islands};
pub use cups::{MIN_CUP_MM2, MIN_CUP_MM3};
pub use layer::{Cured, LayerMeasure, Piece, Stretch, cure, equivalent_disc_mm, erase, measure};
pub use stack::{
    MIN_THIN_RUN_MM, Measured, NECK_LIMIT_MPA, PEEL_LIMIT_N, PEEL_N_PER_MM4, Peak, Risk, RiskKind,
    THIN_MM,
};

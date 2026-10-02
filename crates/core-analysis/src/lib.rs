//! Reading the stack as it is written: what each layer cures, how hard it pulls on the
//! film as the plate lifts it, and where a print is likely to fail.

mod contact;
mod layer;
mod stack;

pub use contact::{MIN_ISLAND_MM2, island_runs, islands};
pub use layer::{Cured, LayerMeasure, Piece, Stretch, cure, equivalent_disc_mm, erase, measure};
pub use stack::{Measured, NECK_LIMIT_MPA, PEEL_LIMIT_N, PEEL_N_PER_MM4, Peak, Risk, RiskKind};

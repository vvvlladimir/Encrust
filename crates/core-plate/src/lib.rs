//! Where a model stands on the build plate.
//!
//! Orienting asks which way up a model prints best and answers with a rotation; arranging
//! asks where several models go and answers with an offset each. Both are explained in
//! `docs/design/orientation.md`.

mod arrange;
mod candidates;
mod error;
mod mask;
mod orient;
mod score;

pub use arrange::{ArrangeSettings, Arranged, Placed, arrange};
pub use error::PlateError;
pub use mask::Footprint;
pub use orient::{OrientSettings, Oriented, orient, rotation_for};
pub use score::Score;

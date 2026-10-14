//! Where a model stands on the build plate.
//!
//! Orienting asks which way up a model prints best and answers with a rotation; arranging
//! asks where several models go and answers with an offset each. Both are explained in
//! `docs/design/orientation.md`.

mod arrange;
mod candidates;
mod cups;
mod cut;
mod error;
mod low_points;
mod mask;
mod orient;
mod printed;
mod score;
mod section;
mod select;

pub use arrange::{ArrangeSettings, Arranged, Placed, arrange};
pub use candidates::{FlatFace, flat_faces};
pub use error::PlateError;
pub use mask::Footprint;
pub use orient::{OrientSettings, Oriented, finalists, orient, orient_printed, rotation_for};
pub use printed::Printed;
pub use score::{OrientGoal, Score};

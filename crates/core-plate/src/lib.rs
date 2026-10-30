//! Where a model stands on the build plate: orienting answers with a rotation, arranging with an
//! offset each. See `docs/design/orientation.md`.

mod arrange;
mod candidates;
mod cups;
mod cut;
mod error;
mod fit;
mod low_points;
mod mask;
mod orient;
mod policy;
mod printed;
mod score;
mod section;
mod select;
mod spot;
mod watch;

pub use arrange::{ArrangeSettings, Arranged, Placed, arrange};
pub use candidates::{FlatFace, flat_faces};
pub use error::PlateError;
pub use mask::Footprint;
pub use orient::{OrientSettings, Oriented, finalists, orient, orient_printed, rotation_for};
pub use policy::{Noise, Readings, Ruling, rank, rulings};
pub use printed::Printed;
pub use score::{LOOKS_UP, OrientGoal, OrientPreset, Score};
pub use spot::{Rect, nearest_free};
pub use watch::{Phase, Pieces, Watch};

//! Pins, dowels, magnets and keys that join the two halves of a cut model back true.
//!
//! The cut face is drawn on a grid with its distance to the edge, fixings are spread over
//! each separate piece of it, sounded for depth either side, and built into the halves by
//! exact booleans; see `docs/design/joints.md`.

mod band;
mod distance;
mod error;
mod join;
mod key;
mod pegs;
mod section;
mod settings;
mod shapes;
mod spots;

pub use error::JointError;
pub use join::{CutJoined, Dowel, Halves, Joined, cut_and_join, join};
pub use settings::{JointKind, Joints, Key, Magnets, Pegs};

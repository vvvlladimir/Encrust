//! What is asked of a plate, as data: one [`Edit`] per action, carried out by
//! `core-document` (ADR 0466, `docs/design/document.md`).

mod bridge;
mod edit;

pub use bridge::bridge_edges;
pub use core_engine::project::{Axis, Keep, ObjectId};
pub use core_joint::{JointKind, Joints};
pub use core_volume::{HoleKind, PLUG_GAP_MM};
pub use edit::{AUTO, Edit, Op, OrientAim};

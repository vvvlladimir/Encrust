//! What is asked of a plate, as data: one [`Edit`] per action, carried out by
//! `core-document` (ADR 0466, `docs/design/document.md`).

mod bridge;
pub mod defaults;
mod edit;

pub use bridge::bridge_edges;
pub use core_engine::project::{Axis, Keep, ObjectId};
pub use core_joint::{JointAsked, JointKind, Joints};
/// How `Edit::Combine` joins its objects.
pub use core_volume::Boolean as Op;
pub use core_volume::{HoleKind, PLUG_GAP_MM};
pub use edit::{AUTO, Edit, OrientAim};

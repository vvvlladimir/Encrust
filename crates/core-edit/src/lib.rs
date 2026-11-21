//! What is asked of a plate, as data: one [`Edit`] per action, carried out by
//! `core-document` (ADR 0466, `docs/design/document.md`).

mod edit;

pub use core_engine::project::{Axis, Keep, ObjectId};
pub use core_joint::{JointKind, Joints};
pub use core_volume::PLUG_GAP_MM;
pub use edit::{AUTO, Edit, Op, OrientAim};

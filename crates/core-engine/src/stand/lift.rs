use std::sync::Arc;

use core_geometry::{Aabb, Bvh, Mesh, Scalar, lift_over_plate};

use super::{SupportPlan, needs_under};
use crate::error::EngineError;

/// Nearer than this a model's lowest point rests on what is under it, millimetres.
const RESTING_MM: Scalar = 1e-3;

/// Where [`super::stand_under`], or [`super::stand_seated`], will stand a model, known before
/// anything is cut into it: a point given on the model as it prints is this much higher.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lift {
    /// How far the model goes up, millimetres.
    pub lifted_mm: Scalar,
    /// Whether its lowest point then rests on the plate or the raft's top, sealing what opens there.
    pub resting: bool,
}

impl Lift {
    /// A model standing on the plate where it is, on no supports.
    pub const ON_THE_PLATE: Self = Self {
        lifted_mm: 0.0,
        resting: true,
    };
}

/// How the plan will stand `mesh`, seated on the plate or not, by the rule standing it follows.
/// A seated model goes up onto a raft only when something on it hangs, so that is looked for.
pub fn planned_lift(
    mesh: &Mesh,
    plan: &SupportPlan,
    seated: bool,
    layer_height_mm: Scalar,
) -> Result<Lift, EngineError> {
    let bounds = mesh.aabb().ok_or(EngineError::EmptyPlate)?;
    let placing = plan.placing();
    let raft_mm = if placing.raft.enabled {
        placing.raft.thickness_mm
    } else {
        0.0
    };
    if !seated {
        let lifted_mm = lift_of(&bounds, placing.lift_mm());
        return Ok(Lift {
            lifted_mm,
            resting: bounds.mins.z + lifted_mm <= raft_mm + RESTING_MM,
        });
    }
    let mut lifted_mm = lift_of(&bounds, 0.0);
    if placing.raft.enabled {
        let bvh = Arc::new(Bvh::build(mesh));
        if !needs_under(mesh, &bvh, None, plan, (layer_height_mm, false))?.is_empty() {
            lifted_mm += raft_mm;
        }
    }
    Ok(Lift {
        lifted_mm,
        resting: true,
    })
}

/// How far a model in `bounds` goes up to stand `lift_mm` clear of the plate: not at all when
/// it already stands that high, since the lift makes room under a part, it does not place it.
pub(super) fn lift_of(bounds: &Aabb, lift_mm: Scalar) -> Scalar {
    if bounds.mins.z >= lift_mm {
        return 0.0;
    }
    lift_over_plate(bounds, lift_mm).z
}

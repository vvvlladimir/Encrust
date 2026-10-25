//! The support ghost kept from one frame to the next: a resting cursor would otherwise sweep
//! clearances and hand the card a new mesh every frame.

use std::sync::Arc;

use core_geometry::{PlacedHit, Scalar, Transform, Vec3};
use printer_profiles::SupportProfile;

use crate::panels::Window;
use crate::render::Ghost;
use crate::scene::ObjectId;

use super::supports::{line_ghost, line_start, support_ghost};

/// Everything a support ghost is built from; the same inputs build the same ghost.
#[derive(Clone, PartialEq)]
struct Inputs {
    id: ObjectId,
    point: Vec3,
    line: bool,
    from: Option<Vec3>,
    model: usize,
    surface: usize,
    transform: Transform,
    revision: u64,
    table: Vec<SupportProfile>,
    active: u16,
    spacing_mm: Scalar,
}

impl Inputs {
    fn of(window: &Window, id: ObjectId, hit: &PlacedHit, line: bool) -> Option<Self> {
        let object = window.doc.scene.get(id)?;
        let supports = &window.tools.supports;
        Some(Self {
            id,
            point: hit.point,
            line,
            from: line.then(|| line_start(window, id)).flatten(),
            model: Arc::as_ptr(&object.mesh) as usize,
            surface: Arc::as_ptr(object.support_surface().0) as usize,
            transform: object.transform,
            revision: object.supports.revision(),
            table: supports.table_for(object),
            active: supports.active,
            spacing_mm: supports.inline_spacing_mm,
        })
    }
}

/// The support, or with `line` the line of supports, a click at `hit` on `id` would stand,
/// built again only when one of its [`Inputs`] changed.
pub(super) fn cached_support_ghost(
    ctx: &egui::Context,
    window: &Window,
    id: ObjectId,
    hit: &PlacedHit,
    line: bool,
) -> Option<Ghost> {
    let inputs = Inputs::of(window, id, hit, line)?;
    let store = egui::Id::new("support-ghost");
    let kept = ctx.data(|data| data.get_temp::<(Inputs, Option<Ghost>)>(store));
    if let Some((_, ghost)) = kept.filter(|(held, _)| *held == inputs) {
        return ghost;
    }
    let ghost = if line {
        line_ghost(window, id, hit)
    } else {
        support_ghost(window, id, hit)
    };
    ctx.data_mut(|data| data.insert_temp(store, (inputs, ghost.clone())));
    ghost
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::GhostShape;
    use crate::state::{Doc, Machine, Tools, View};
    use crate::supports::tests::one_supported_cube;
    use crate::workspace::{Mode, Tool};

    fn mesh_of(ghost: Option<Ghost>) -> Arc<core_geometry::Mesh> {
        match ghost.expect("a support stands under the cube").shape {
            GhostShape::Body { mesh, .. } => mesh,
            GhostShape::Hole { .. } => panic!("a support ghost is a body"),
        }
    }

    fn under_the_cube(x: Scalar) -> PlacedHit {
        PlacedHit {
            point: Vec3::new(x, 0.3, 10.0),
            normal: Vec3::NEG_Z,
            distance: 1.0,
            face: 0,
        }
    }

    #[test]
    fn a_resting_cursor_keeps_its_ghost_and_a_moved_one_builds_another() {
        let mut doc = Doc {
            scene: one_supported_cube(),
            ..Doc::default()
        };
        let (mut mode, mut tool) = (Mode::default(), Tool::Paint);
        let (mut view, mut tools, mut machine) = (
            View::framing(&doc.plate),
            Tools::default(),
            Machine::default(),
        );
        let window = Window {
            mode: &mut mode,
            tool: &mut tool,
            doc: &mut doc,
            view: &mut view,
            tools: &mut tools,
            machine: &mut machine,
        };
        let id = window.doc.scene.objects()[0].id;
        let ctx = egui::Context::default();
        let ghost = |x| {
            mesh_of(cached_support_ghost(
                &ctx,
                &window,
                id,
                &under_the_cube(x),
                false,
            ))
        };

        let first = ghost(0.3);
        assert!(
            Arc::ptr_eq(&first, &ghost(0.3)),
            "the same hit is not built again"
        );
        assert!(!Arc::ptr_eq(&first, &ghost(0.7)), "a moved hit is");
    }
}

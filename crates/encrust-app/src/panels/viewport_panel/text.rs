use std::sync::Arc;

use core_geometry::{PlacedHit, Transform};

use super::picking::surface_under;
use crate::panels::Window;
use crate::render::{Ghost, GhostShape};
use crate::scene::ObjectId;
use crate::ui::theme;

/// Stamps the label into the model where it was clicked, and selects that model.
pub(super) fn stamp_text_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) {
    if window.tools.text.is_busy() {
        return;
    }
    let Some((id, hit)) = surface_under(window, viewport, cursor) else {
        return;
    };
    window.doc.scene.select(Some(id));
    let started = window.tools.text.stamp(&window.doc.scene, id, &hit);
    window
        .machine
        .status
        .report(&crate::i18n::tr!("text-stamping"), started);
}

/// The label a click at `hit` on object `id` would stamp, as it would stand.
pub(super) fn text_ghost(window: &Window, id: ObjectId, hit: &PlacedHit) -> Option<Ghost> {
    let object = window.doc.scene.get(id)?;
    let label = window.tools.text.label_at(object, hit).ok()?;
    Some(Ghost {
        shape: GhostShape::Body {
            mesh: Arc::new(crate::text::ghost_mesh(&label)),
            transform: Transform::default(),
        },
        colour: theme::colors().accent,
    })
}

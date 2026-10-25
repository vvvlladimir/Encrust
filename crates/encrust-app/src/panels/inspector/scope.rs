use crate::i18n::tr;
use crate::panels::Window;
use crate::scene::{ObjectId, SceneObject};
use crate::state::Tools;
use crate::ui::hint;

/// Where a tool keeps its numbers: the plate's in the tool, a model's own on the model.
pub struct Scoped<T> {
    pub plate: fn(&mut Tools) -> &mut T,
    pub own: fn(&mut SceneObject) -> &mut Option<T>,
    pub read: fn(&SceneObject) -> &Option<T>,
}

/// The one picked model whose own numbers the panel edits. With none or several picked the
/// panel edits the plate's, and says how many models keep their own. See ADR 0319.
pub fn owner<T>(ui: &mut egui::Ui, window: &Window, scoped: &Scoped<T>) -> Option<ObjectId> {
    let picked = match window.doc.scene.selection() {
        [id] => Some(*id),
        _ => None,
    };
    let keeping = window
        .doc
        .scene
        .here()
        .filter(|object| (scoped.read)(object).is_some())
        .count();
    if picked.is_none() && keeping > 0 {
        hint(ui, &tr!("scope-keeping", count = keeping));
    }
    picked
}

/// Runs `draw` with the tool holding `id`'s own numbers, keeping what it changed on the model;
/// without a model, `draw` edits the plate's. The model keeps its numbers while drawing.
pub fn editing<T: Clone + PartialEq>(
    window: &mut Window,
    id: Option<ObjectId>,
    scoped: &Scoped<T>,
    draw: impl FnOnce(&mut Window),
) {
    let Some(object) = id.and_then(|id| window.doc.scene.get(id)) else {
        draw(window);
        return;
    };
    let plate = (scoped.plate)(window.tools).clone();
    let own = (scoped.read)(object).clone();
    let shown = own.clone().unwrap_or_else(|| plate.clone());
    *(scoped.plate)(window.tools) = shown.clone();
    draw(window);
    let edited = std::mem::replace((scoped.plate)(window.tools), plate);
    if (own.is_some() || edited != shown)
        && let Some(object) = id.and_then(|id| window.doc.scene.get_mut(id))
    {
        *(scoped.own)(object) = Some(edited);
    }
}

/// Runs `draw` with the Supports tool holding the picked models' profile; edits stay theirs
/// until saved over the profile (`docs/decisions/0246`). `draw` must not change the group.
pub fn editing_supports(window: &mut Window, draw: impl FnOnce(&mut Window)) {
    if window.doc.scene.selection().is_empty() {
        draw(window);
        window.tools.supports.adopt_saved(&mut window.doc.scene);
        window.tools.supports.take_up_chosen(&mut window.doc.scene);
        return;
    }
    let shown = window.tools.supports.shown(&window.doc.scene).clone();
    let plate = std::mem::replace(&mut window.tools.supports.profile, shown.clone());
    draw(window);
    let edited = std::mem::replace(&mut window.tools.supports.profile, plate);
    if edited != shown {
        window
            .tools
            .supports
            .keep_on_picked(&edited, &mut window.doc.scene);
    }
    window.tools.supports.adopt_saved(&mut window.doc.scene);
    window.tools.supports.take_up_chosen(&mut window.doc.scene);
}

#[cfg(test)]
mod tests;

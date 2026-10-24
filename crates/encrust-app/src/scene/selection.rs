use super::object::SceneObject;
use super::{ObjectId, Scene};
use crate::i18n::tr;

impl Scene {
    /// The object the inspector is talking about: the last one picked.
    pub fn selected(&self) -> Option<ObjectId> {
        self.selected.last().copied()
    }

    /// Everything picked, in the order it was picked.
    pub fn selection(&self) -> &[ObjectId] {
        &self.selected
    }

    pub fn is_selected(&self, id: ObjectId) -> bool {
        self.selected.contains(&id)
    }

    /// What a tool works on: the selection, or the whole plate when nothing is picked.
    ///
    /// Only what is on the plate being edited and not hidden ever comes back, so a stale
    /// pick cannot reach a model the user is not looking at. See `docs/decisions/0101`.
    pub fn targets(&self) -> impl Iterator<Item = &SceneObject> {
        let taking_all = self.selected.is_empty();
        self.printable(self.active)
            .filter(move |object| taking_all || self.selected.contains(&object.id))
    }

    /// The same, to be edited.
    pub fn targets_mut(&mut self) -> impl Iterator<Item = &mut SceneObject> {
        let active = self.active;
        let selected = self.selected.clone();
        let taking_all = selected.is_empty();
        self.objects.iter_mut().filter(move |object| {
            object.plate == active
                && object.visible
                && (taking_all || selected.contains(&object.id))
        })
    }

    /// How many models a tool would act on.
    pub fn target_count(&self) -> usize {
        self.targets().count()
    }

    /// Whether a tool would act on a subset of the plate rather than all of it, which is
    /// what the buttons say out loud.
    pub fn has_selection(&self) -> bool {
        !self.selected.is_empty()
    }

    /// What a tool is about to work on, for the line under its button.
    pub fn scope(&self) -> String {
        let count = self.target_count();
        match (self.has_selection(), count) {
            (true, 1) => tr!("scope-selected-one"),
            (true, count) => tr!("scope-selected", count = count),
            (false, 1) => tr!("scope-plate-one"),
            (false, count) => tr!("scope-plate", count = count),
        }
    }

    /// Replaces the selection. Picking an object that is not in the scene clears it.
    pub fn select(&mut self, id: Option<ObjectId>) {
        self.selected = id
            .filter(|id| self.get(*id).is_some())
            .into_iter()
            .collect();
        self.anchor = self.selected.first().copied();
    }

    /// Adds an object to the selection, or takes it out if it is already in: what a
    /// modifier-click does in every application that has more than one thing to pick.
    pub fn toggle_selected(&mut self, id: ObjectId) {
        if self.get(id).is_none() {
            return;
        }
        match self.selected.iter().position(|picked| *picked == id) {
            Some(at) => {
                self.selected.remove(at);
            }
            None => self.selected.push(id),
        }
        self.anchor = Some(id);
    }

    /// Picks everything standing between the model a span was last started from and `id`,
    /// in the order the plate lists them: what a shift-click does in a list.
    ///
    /// The span is measured from the same model however often it is redrawn, so dragging
    /// the shift-click back over the list shrinks the selection rather than growing it.
    pub fn select_span_to(&mut self, id: ObjectId) {
        let Some(anchor) = self.anchor.filter(|anchor| self.get(*anchor).is_some()) else {
            return self.select(Some(id));
        };
        let here: Vec<ObjectId> = self.here().map(|object| object.id).collect();
        let (Some(from), Some(to)) = (
            here.iter().position(|other| *other == anchor),
            here.iter().position(|other| *other == id),
        ) else {
            return self.select(Some(id));
        };
        let span = if from <= to { from..=to } else { to..=from };
        self.selected = here[span].to_vec();
        self.anchor = Some(anchor);
    }

    /// Picks exactly these, dropping anything that is not in the scene.
    pub fn select_many(&mut self, ids: &[ObjectId]) {
        self.selected = ids
            .iter()
            .copied()
            .filter(|id| self.get(*id).is_some())
            .collect();
        self.anchor = self.selected.first().copied();
    }

    /// Takes every picked model off the plate. Returns how many went.
    pub fn remove_selected(&mut self) -> usize {
        self.anchor = None;
        let going = std::mem::take(&mut self.selected);
        self.objects.retain(|object| !going.contains(&object.id));
        going.len()
    }

    /// Picks everything on the plate being edited.
    pub fn select_here(&mut self) {
        self.selected = self.here().map(|object| object.id).collect();
        self.anchor = self.selected.first().copied();
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }
}

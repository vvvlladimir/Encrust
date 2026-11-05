//! The recipes Prepare for print can take, by id, and the words a plan names their steps and
//! notes by (ADR 0405).

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use anyhow::{Result, bail};
use core_recipe::{Listed, Note, Plan, Read, StepKind};
use printer_profiles::Catalogue;

use crate::i18n::tr;

/// The recipes the picker offers, read again only once the user's texts change.
#[derive(Debug, Default)]
pub struct Library {
    read: RefCell<Option<(u64, Arc<Vec<Listed>>)>>,
}

impl Library {
    /// The built-in recipes, then the user's that no built-in id hides (ADR 0405).
    pub fn listed(&self, catalogue: &Catalogue) -> Arc<Vec<Listed>> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for entry in catalogue.recipes() {
            (&entry.id, &entry.profile).hash(&mut hasher);
        }
        let key = hasher.finish();
        let mut read = self.read.borrow_mut();
        match &*read {
            Some((kept, listed)) if *kept == key => Arc::clone(listed),
            _ => {
                let listed = Arc::new(recipes(catalogue));
                *read = Some((key, Arc::clone(&listed)));
                listed
            }
        }
    }

    /// The plan of the recipe `id`, or why it cannot run.
    pub fn plan(&self, catalogue: &Catalogue, id: &str) -> Result<Plan> {
        let listed = self.listed(catalogue);
        plan_of(listed.iter().find(|entry| entry.id == id), id)
    }
}

fn recipes(catalogue: &Catalogue) -> Vec<Listed> {
    let user = catalogue
        .recipes()
        .map(|entry| (entry.id.as_str(), entry.profile.as_str()));
    let mut listed = core_recipe::library(user);
    listed.retain(|entry| !entry.is_shadowed());
    listed
}

/// The plan of `listed`, the recipe `id` names, or why it cannot run.
fn plan_of(listed: Option<&Listed>, id: &str) -> Result<Plan> {
    let Some(listed) = listed else {
        bail!(tr!("prepare-recipe-missing", recipe = id));
    };
    match &listed.read {
        Read::Valid(plan) => Ok(plan.clone()),
        Read::Invalid { error, .. } | Read::Unreadable(error) => {
            let context = tr!("prepare-recipe-cannot-run", recipe = listed.name());
            Err(anyhow::Error::new(error.clone()).context(context))
        }
    }
}

/// The name a step of `kind` goes by in a plan and in the editor.
pub fn kind_name(kind: StepKind) -> String {
    match kind {
        StepKind::Repair => tr!("recipe-kind-repair"),
        StepKind::Orient => tr!("recipe-kind-orient"),
        StepKind::Hollow => tr!("recipe-kind-hollow"),
        StepKind::Drain => tr!("recipe-kind-drain"),
        StepKind::Supports => tr!("recipe-kind-supports"),
        StepKind::Check => tr!("recipe-kind-check"),
        StepKind::Arrange => tr!("recipe-kind-arrange"),
        StepKind::Save => tr!("recipe-kind-save"),
        StepKind::Gate => tr!("recipe-kind-gate"),
        StepKind::Pause => tr!("recipe-kind-pause"),
    }
}

/// What a plan says under a step that undoes an earlier one, which it names by number.
pub fn note_text(note: &Note) -> String {
    let over = note.over + 1;
    match (note.kind, note.over_kind) {
        (StepKind::Orient, StepKind::Drain) => tr!("recipe-note-orient-drain", step = over),
        (StepKind::Orient, _) => tr!("recipe-note-orient-supports", step = over),
        (StepKind::Hollow, _) => tr!("recipe-note-hollow-supports", step = over),
        (StepKind::Repair, _) => tr!("recipe-note-repair", step = over),
        _ => tr!("recipe-note-supports-again", step = over),
    }
}

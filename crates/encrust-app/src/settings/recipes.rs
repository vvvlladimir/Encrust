//! The user's recipes opened, saved, copied, renamed and thrown away through the profile
//! catalogue (ADR 0405); the picker's choice follows a rename and a deletion.

use anyhow::{Result, bail};
use core_recipe::{Listed, is_built_in};
use printer_profiles::{Catalogue, Kind};

use super::{Draft, RecipeForm, Settings, unique_id};
use crate::i18n::tr;
use crate::prepare::DEFAULT_RECIPE;

impl Settings {
    /// Opens `listed` in the form; one that does not read stays closed.
    pub fn open_recipe(&mut self, listed: &Listed) -> bool {
        self.recipe_pick = Some(listed.id.clone());
        self.recipe = listed
            .recipe()
            .map(|recipe| Draft::new(listed.id.clone(), RecipeForm::of(recipe)));
        self.recipe.is_some()
    }

    /// Whether the recipe open in the form is a built-in one, which is only read.
    pub fn recipe_is_built_in(&self) -> bool {
        self.recipe
            .as_ref()
            .is_some_and(|draft| is_built_in(&draft.id))
    }
}

/// Writes the recipe in the form once it passes its check, under a new stem when its name
/// changed; `chosen`, the picker's recipe, follows it. Answers the id it is kept under.
pub fn save_recipe(
    settings: &mut Settings,
    catalogue: &mut Catalogue,
    chosen: &mut String,
) -> Result<String> {
    let Some(draft) = settings.recipe.as_mut() else {
        bail!(tr!("recipes-nothing-open"));
    };
    if is_built_in(&draft.id) {
        bail!(tr!("recipes-built-in-why"));
    }
    let plan = draft.values.check().map_err(|checked| {
        let first = checked.fields.into_iter().next().map(|(_, error)| error);
        match checked.whole.map(|(_, error)| error).or(first) {
            Some(error) => anyhow::Error::new(error),
            None => anyhow::anyhow!(tr!("recipes-cannot-save")),
        }
    })?;
    let was = draft.id.clone();
    let name = &plan.recipe().name;
    let id = match unique_id(name, |_| false) {
        same if same == was => was.clone(),
        _ => catalogue.fresh_recipe_id(name, is_built_in),
    };
    catalogue.move_recipe(&was, &id, &plan.recipe().to_toml())?;
    if *chosen == was {
        chosen.clone_from(&id);
    }
    draft.id.clone_from(&id);
    draft.mark_saved();
    settings.recipe_pick = Some(id.clone());
    Ok(id)
}

/// Keeps a copy of `listed` as one of the user's recipes, named apart from it, and opens it.
/// Answers the copy's id.
pub fn duplicate_recipe(
    settings: &mut Settings,
    catalogue: &mut Catalogue,
    listed: &Listed,
) -> Result<String> {
    let Some(recipe) = listed.recipe() else {
        bail!(tr!("recipes-unreadable-why"));
    };
    let mut copy = recipe.clone();
    copy.name = tr!("recipes-copy-name", name = recipe.name.as_str());
    let id = catalogue.fresh_recipe_id(&copy.name, is_built_in);
    catalogue.save_recipe(&id, &copy.to_toml())?;
    settings.recipe_pick = Some(id.clone());
    settings.recipe = Some(Draft::new(id.clone(), RecipeForm::of(&copy)));
    Ok(id)
}

/// Throws the user's recipe `id` away; the picker goes back to the built-in one if it had it.
pub fn delete_recipe(
    settings: &mut Settings,
    catalogue: &mut Catalogue,
    id: &str,
    chosen: &mut String,
) -> Result<()> {
    if is_built_in(id) {
        bail!(tr!("recipes-built-in-why"));
    }
    catalogue.forget_user_copy(Kind::Recipe, id)?;
    if chosen == id {
        *chosen = DEFAULT_RECIPE.to_owned();
    }
    settings.recipe_pick = None;
    settings.recipe = None;
    Ok(())
}

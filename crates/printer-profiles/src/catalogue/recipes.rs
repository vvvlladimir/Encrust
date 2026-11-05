//! The user's recipes, kept as their text under their file stem: `core-recipe` reads and
//! checks them, this crate only lists and writes them (ADR 0405).

use std::path::PathBuf;

use super::{Catalogue, Entry, Kind, Source};
use crate::ProfileError;
use crate::exchange::unique_id;

impl Catalogue {
    /// Every recipe of the user's, by id, as the text kept.
    pub fn recipes(&self) -> impl Iterator<Item = &Entry<String>> {
        self.recipes.values()
    }

    pub fn recipe(&self, id: &str) -> Result<&Entry<String>, ProfileError> {
        self.recipes.get(id).ok_or_else(|| ProfileError::Unknown {
            kind: Kind::Recipe.label(),
            id: id.to_owned(),
        })
    }

    /// Writes `text` as the user's recipe `id`, over any kept under it.
    pub fn save_recipe(&mut self, id: &str, text: &str) -> Result<PathBuf, ProfileError> {
        let path = self.keep(Kind::Recipe, id, |_| Ok(text.to_owned()))?;
        self.recipes.insert(
            id.to_owned(),
            Entry {
                id: id.to_owned(),
                profile: text.to_owned(),
                source: Source::User(path.clone()),
            },
        );
        Ok(path)
    }

    /// An id for a new recipe called `name`: its stem, numbered past those kept and those
    /// `reserved`, the ids that ship in `core-recipe`, which a lookup finds first.
    pub fn fresh_recipe_id(&self, name: &str, reserved: impl Fn(&str) -> bool) -> String {
        unique_id(name, |id| self.is_taken(Kind::Recipe, id) || reserved(id))
    }

    /// Keeps `text` under `to` and throws away what was kept under `from`: a rename.
    pub fn move_recipe(&mut self, from: &str, to: &str, text: &str) -> Result<(), ProfileError> {
        self.save_recipe(to, text)?;
        if from != to {
            self.forget_user_copy(Kind::Recipe, from)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn writable(name: &str) -> (Catalogue, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("encrust-recipes-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let catalogue = Catalogue::with_root(&dir).expect("a missing directory reads as empty");
        (catalogue, dir)
    }

    #[test]
    fn a_saved_recipe_is_a_file_in_recipes_and_reads_back() {
        let (mut catalogue, dir) = writable("saved");
        let path = catalogue
            .save_recipe("busts", "name = \"Busts\"\n")
            .expect("the directory is writable");
        assert_eq!(path, dir.join("recipes").join("busts.toml"));
        let again = Catalogue::with_root(&dir).expect("what was written reads");
        let entry = again.recipe("busts").expect("kept");
        assert_eq!(entry.profile, "name = \"Busts\"\n");
    }

    #[test]
    fn a_new_id_passes_over_kept_and_reserved_ones() {
        let (mut catalogue, _) = writable("fresh");
        catalogue
            .save_recipe("busts", "name = \"Busts\"\n")
            .expect("writable");
        assert_eq!(catalogue.fresh_recipe_id("Busts", |_| false), "busts-2");
        assert_eq!(
            catalogue.fresh_recipe_id("Prepare", |id| id == "prepare"),
            "prepare-2"
        );
    }

    #[test]
    fn a_moved_recipe_leaves_its_old_stem() {
        let (mut catalogue, dir) = writable("moved");
        catalogue
            .save_recipe("busts", "name = \"Busts\"\n")
            .expect("writable");
        catalogue
            .move_recipe("busts", "heads", "name = \"Heads\"\n")
            .expect("writable");
        assert!(catalogue.recipe("busts").is_err());
        assert!(!dir.join("recipes").join("busts.toml").exists());
        assert!(catalogue.recipe("heads").is_ok());
    }

    #[test]
    fn an_unknown_recipe_is_unknown() {
        let (catalogue, _) = writable("unknown");
        assert!(matches!(
            catalogue.recipe("busts"),
            Err(ProfileError::Unknown { kind: "recipe", .. })
        ));
    }
}

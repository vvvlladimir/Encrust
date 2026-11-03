//! What of a recipe the plate does not show, in English for the model to read.

use super::recipe::Recipe;

impl Recipe {
    /// Each part of the recipe the window does its own way, one line each: a plug laid beside
    /// every hole, and a whole plate arranged where the command line arranges the job's models.
    pub(super) fn unshown(&self) -> Vec<String> {
        let mut said = Vec::new();
        let drilled = self.drains.as_ref().is_some_and(|drains| {
            drains.auto || !drains.at_mm.is_empty() || !drains.support_at_mm.is_empty()
        });
        if drilled && !self.drains.as_ref().is_some_and(|drains| drains.plugs) {
            said.push("plugs: the window lays a plug beside every hole".to_owned());
        }
        if self
            .modelling
            .as_ref()
            .is_some_and(|modelling| modelling.arrange)
        {
            said.push("arrange: the window arranges every model on its plate".to_owned());
        }
        said
    }
}

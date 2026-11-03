//! What of a recipe the plate does not show, in English for the model to read: the steps the
//! window has no replay for yet, and where it does one its own way.

use super::recipe::Recipe;

impl Recipe {
    /// Each part of the recipe the window leaves out or does differently, one line each.
    pub(super) fn unshown(&self) -> Vec<String> {
        let mut said = Vec::new();
        let mut say = |part: &str| said.push(format!("{part}: not shown in the window yet"));
        // TODO(step-8): replay the modelling, relief, mend and slicing through the window's own
        // Boolean, Text, Cut, Relief and Repair tools and its slicing settings.
        if let Some(modelling) = &self.modelling {
            let parts = [
                ("combine", modelling.combine.is_some()),
                ("label", modelling.label.is_some()),
                ("cut", modelling.cut.is_some()),
                ("split", modelling.split_smallest_mm3.is_some()),
                ("part_off_at_mm", modelling.part_off_at_mm.is_some()),
            ];
            for (part, given) in parts {
                if given {
                    say(part);
                }
            }
        }
        for (part, given) in [
            ("relief", self.relief.is_some()),
            ("mend", self.mend.is_some()),
            ("slicing", self.slicing.is_some()),
        ] {
            if given {
                say(part);
            }
        }
        if let Some(drains) = &self.drains {
            for (part, given) in [
                ("channels", !drains.channels.is_empty()),
                ("support_at_mm", !drains.support_at_mm.is_empty()),
                ("walled", drains.kind.as_deref() == Some("walled")),
            ] {
                if given {
                    say(part);
                }
            }
        }
        said
    }
}

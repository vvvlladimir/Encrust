//! Leaving a support profile's form that holds edits not yet saved: the way out waits in
//! `Settings::leaving` until the question over it is answered (ADR 0389).

use super::{Draft, Section, Settings};

/// Where the form was being left for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaving {
    /// Back to the table of every profile.
    Back,
    Close,
    Turn(Section),
    /// The sheet of keys, which the window closes to show.
    Keys,
    /// The Machine and resin window, which the window closes to show.
    Machines,
}

impl Settings {
    /// Whether the support profile open in the form holds edits not yet saved.
    pub fn unsaved(&self) -> bool {
        self.editing_support
            && match &self.auto {
                Some(auto) => auto.is_dirty(),
                None => self.support.as_ref().is_some_and(Draft::is_dirty),
            }
    }

    /// Goes where `to` leads, or keeps it for the question while the form holds unsaved
    /// edits. Answers whether it went.
    pub fn leave(&mut self, to: Leaving) -> bool {
        if self.unsaved() {
            self.leaving = Some(to);
            return false;
        }
        self.go(to);
        true
    }

    /// Goes where `to` leads, whatever the form holds.
    pub fn go(&mut self, to: Leaving) {
        self.leaving = None;
        match to {
            Leaving::Back => self.editing_support = false,
            Leaving::Close | Leaving::Keys | Leaving::Machines => self.shut(),
            Leaving::Turn(section) => self.turn_to(section),
        }
    }

    /// Puts the profile open in the form back as it was last saved.
    pub fn discard(&mut self) {
        match (&mut self.auto, &mut self.support) {
            (Some(auto), _) => auto.discard(),
            (None, Some(support)) => support.discard(),
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use printer_profiles::Catalogue;

    /// The window open on Heavy's form, its pillar widened and not saved.
    fn editing_heavy(edited: bool) -> Settings {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue loads");
        let mut settings = Settings {
            open: true,
            editing_support: true,
            ..Settings::default()
        };
        settings.pick_support(&catalogue, "heavy");
        if edited {
            let draft = settings.support.as_mut().expect("Heavy ships");
            draft.values.middle.diameter_mm += 0.5;
        }
        settings
    }

    #[test]
    fn closing_a_form_with_unsaved_edits_waits_for_the_answer() {
        let mut settings = editing_heavy(true);
        settings.close();
        assert!(
            settings.open,
            "the window stays until the question is answered"
        );
        assert_eq!(settings.leaving, Some(Leaving::Close));
    }

    #[test]
    fn a_form_with_nothing_unsaved_is_left_at_once() {
        let mut settings = editing_heavy(false);
        assert!(settings.leave(Leaving::Turn(Section::General)));
        assert_eq!(settings.section, Section::General);
        assert_eq!(settings.leaving, None);
    }

    #[test]
    fn discarding_puts_the_form_back_as_last_saved() {
        let mut settings = editing_heavy(true);
        settings.discard();
        assert!(!settings.unsaved());
        settings.go(Leaving::Back);
        assert!(!settings.editing_support, "back on the table");
    }
}

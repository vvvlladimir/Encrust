//! What Check's fixes last did, by the kind of issue each answered: said on the kind's own
//! card for as long as the edit that did it is the plate's latest word (ADR 0416).

use serde::{Deserialize, Serialize};

use crate::status::Status;
use crate::undo::History;

/// A kind of issue Check gives a card of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueKind {
    InTheWay,
    KeepsResin,
    Islands,
    Cups,
    Necks,
    Peel,
    Thin,
}

/// What one fix did to one kind of issue, and the edit that holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub kind: IssueKind,
    pub said: Status,
    edit: u64,
}

/// The fixes made, oldest first.
#[derive(Debug, Default)]
pub struct Answers(Vec<Answer>);

impl Answers {
    /// Notes that the edit about to be recorded answers `kind` with `said`; a fix that
    /// changed nothing is no answer.
    pub fn note(&mut self, history: &History, changed: bool, kind: IssueKind, said: &Status) {
        if changed {
            self.0.push(Answer {
                kind,
                said: said.clone(),
                edit: history.next_edit(),
            });
        }
    }

    /// The answers that still stand: each one's edit made, and nothing but fixes done since.
    /// An undone answer waits for its redo; one past the history's reach is forgotten.
    pub fn standing(&mut self, history: &History) -> Vec<Answer> {
        self.0.retain(|answer| history.knows(answer.edit));
        let made = |edit: u64| self.0.iter().any(|answer| answer.edit == edit);
        self.0
            .iter()
            .filter(|answer| {
                history
                    .edits_since(answer.edit)
                    .is_some_and(|mut since| since.all(made))
            })
            .cloned()
            .collect()
    }

    /// Whether `answer` is the plate's last edit, so that one undo takes back this and no other.
    pub fn is_last(answer: &Answer, history: &History) -> bool {
        history.last_edit() == Some(answer.edit)
    }
}

#[cfg(test)]
mod tests;

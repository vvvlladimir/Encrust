//! The cards Check lays its issues out on, one to a kind: which there are, which is open, and
//! a card or the line of what answered it drawn for a kind (ADR 0416).

use core_analysis::Risk;
use egui::Color32;

use super::issues::kind_of;
use super::plate::{self, PlateIssue};
use super::tint;
use crate::check::{Answer, Answers, IssueKind};
use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::{IssueHead, answered_row, hint, icon, issue_card, kept, theme};

/// The kinds a stack's layers can show, in the order they are laid out.
pub(super) const LAYER_KINDS: [IssueKind; 5] = [
    IssueKind::Islands,
    IssueKind::Cups,
    IssueKind::Necks,
    IssueKind::Peel,
    IssueKind::Thin,
];

/// One kind with something still to answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Card {
    pub kind: IssueKind,
    pub count: usize,
    /// How many of `count` Fix issues takes on.
    pub fixable: usize,
    pub tint: Color32,
}

/// What a read of the stack found.
pub(super) struct Layers {
    pub risks: Vec<Risk>,
    pub removed: usize,
}

/// All Check shows this frame.
pub(super) struct Board {
    pub plate: Vec<PlateIssue>,
    pub layers: Option<Layers>,
    pub answers: Vec<Answer>,
    pub cards: Vec<Card>,
    pub open: Option<IssueKind>,
}

impl Board {
    pub(super) fn read(ctx: &egui::Context, window: &mut Window) -> Self {
        let plate = plate::issues(ctx, window);
        let layers = window.measured().map(|measured| Layers {
            risks: measured.risks(),
            removed: measured.removed_islands(),
        });
        let history = &window.doc.history;
        let answers = window.tools.check.answers.standing(history);
        let answered: Vec<IssueKind> = answers.iter().map(|answer| answer.kind).collect();
        let mut cards = plate::cards(window, &plate, &answered);
        if let Some(layers) = &layers {
            cards.extend(layer_cards(&layers.risks, layers.removed, &answered));
        }
        if let Some(kind) = window.tools.check.take_reveal() {
            kept::set(ctx, open_id(), &Some(kind));
        }
        let kept = kept::get::<Option<IssueKind>>(ctx, open_id());
        let open = open_card(kept, &cards);
        Self {
            plate,
            layers,
            answers,
            cards,
            open,
        }
    }

    pub(super) fn has(&self, kind: IssueKind) -> bool {
        self.cards.iter().any(|card| card.kind == kind)
    }

    fn card(&self, kind: IssueKind) -> Option<&Card> {
        self.cards.iter().find(|card| card.kind == kind)
    }

    pub(super) fn answer(&self, kind: IssueKind) -> Option<&Answer> {
        self.answers.iter().rev().find(|answer| answer.kind == kind)
    }

    /// How many issues Fix issues takes on, and how many it leaves to the hand.
    pub(super) fn tally(&self) -> (usize, usize) {
        let fixable: usize = self.cards.iter().map(|card| card.fixable).sum();
        let count: usize = self.cards.iter().map(|card| card.count).sum();
        (fixable, count - fixable)
    }

    fn aside(&self, kind: IssueKind) -> String {
        if kind == IssueKind::Thin {
            return tr!("check-card-by-hand");
        }
        let Some(layers) = &self.layers else {
            return String::new();
        };
        let of_kind = layers
            .risks
            .iter()
            .filter(|risk| kind_of(&risk.kind) == kind);
        let shown: Vec<usize> = of_kind.map(|risk| risk.layer + 1).collect();
        match (shown.iter().min(), shown.iter().max()) {
            (Some(first), Some(last)) if first == last => {
                tr!("check-layer-lower", layer = *first)
            }
            (Some(first), Some(last)) => tr!("check-card-layers", first = *first, last = *last),
            _ => String::new(),
        }
    }
}

fn open_id() -> egui::Id {
    egui::Id::new("check-open-card")
}

/// The card to stand open: the one chosen while it has anything to answer, else the first; none
/// if the user folded the last one chosen.
fn open_card(kept: Option<Option<IssueKind>>, cards: &[Card]) -> Option<IssueKind> {
    let has = |kind| cards.iter().any(|card| card.kind == kind);
    match kept {
        Some(None) => None,
        Some(Some(kind)) if has(kind) => Some(kind),
        _ => cards.first().map(|card| card.kind),
    }
}

/// The kinds `risks` show, each `answered` kind left to the hand as its fix has had its turn.
fn layer_cards(risks: &[Risk], removed: usize, answered: &[IssueKind]) -> Vec<Card> {
    let colors = theme::colors();
    LAYER_KINDS
        .into_iter()
        .filter_map(|kind| {
            let of_kind: Vec<&Risk> = risks
                .iter()
                .filter(|risk| kind_of(&risk.kind) == kind)
                .collect();
            let count = of_kind.len();
            if count == 0 && !(kind == IssueKind::Islands && removed > 0) {
                return None;
            }
            let automatic = matches!(
                kind,
                IssueKind::Islands | IssueKind::Cups | IssueKind::Necks
            );
            Some(Card {
                kind,
                count,
                fixable: if automatic && !answered.contains(&kind) {
                    count
                } else {
                    0
                },
                tint: of_kind.first().map_or(colors.text_mid, |risk| tint(risk)),
            })
        })
        .collect()
}

/// The name `kind` goes by on its card.
fn name_of(kind: IssueKind) -> String {
    match kind {
        IssueKind::InTheWay => tr!("check-card-way"),
        IssueKind::KeepsResin => tr!("check-card-resin"),
        IssueKind::Islands => tr!("check-filter-islands"),
        IssueKind::Cups => tr!("check-section-cups"),
        IssueKind::Necks => tr!("check-section-levers"),
        IssueKind::Peel => tr!("check-issue-peel"),
        IssueKind::Thin => tr!("check-card-thin"),
    }
}

/// Draws `kind`: its card with what `body` adds, told whether a fix has had its turn, or the
/// line of what answered it, or nothing.
pub(super) fn draw(
    ui: &mut egui::Ui,
    window: &mut Window,
    board: &Board,
    kind: IssueKind,
    body: impl FnOnce(&mut egui::Ui, &mut Window, bool),
) {
    let answer = board.answer(kind);
    let name = name_of(kind);
    let Some(card) = board.card(kind) else {
        if let Some(answer) = answer {
            line_of_answer(ui, window, &name, answer);
        }
        return;
    };
    let colors = theme::colors();
    let aside = board.aside(kind);
    let head = IssueHead {
        glyph: if card.tint == colors.danger {
            icon::ERROR
        } else {
            icon::WARNING
        },
        tint: card.tint,
        name: &name,
        aside: &aside,
        count: card.count,
    };
    let open = board.open == Some(kind);
    let (pressed, _) = issue_card(ui, &head, open, |ui| {
        if let Some(answer) = answer {
            said(ui, answer);
        }
        body(ui, window, answer.is_some());
    });
    if pressed {
        kept::set(ui.ctx(), open_id(), &(!open).then_some(kind));
    }
}

/// What a fix did, over what it left.
fn said(ui: &mut egui::Ui, answer: &Answer) {
    hint(ui, &answer.said.text());
    if let crate::status::Status::Noted(_, facts) = &answer.said {
        for fact in facts {
            hint(ui, fact);
        }
    }
}

fn line_of_answer(ui: &mut egui::Ui, window: &mut Window, name: &str, answer: &Answer) {
    let undoable = Answers::is_last(answer, &window.doc.history);
    let tip = tr!("check-undo-fix");
    let pressed = answered_row(
        ui,
        name,
        &answer.said.text(),
        undoable.then_some(tip.as_str()),
    );
    if pressed {
        window.doc.history.undo(
            &mut window.doc.scene,
            window.tools,
            &mut window.machine.slicing,
        );
    }
}

#[cfg(test)]
mod tests;

//! The steps of the recipe open as a chain: a folding card a step, a + on the rail before,
//! between and after them, a card dragged by its grip to another seam.

use core_recipe::{OnFail, StepKind};
use egui::{DragAndDrop, Id, Rect, Ui, vec2};

use super::step;
use crate::i18n::tr;
use crate::prepare::{kind_glyph, kind_name};
use crate::settings::{Checked, RecipeForm, StepForm};
use crate::ui::{
    ChainStep, ConditionName, DropdownRow, chain_drop_mark, chain_ghost, chain_seam, chain_step,
    dropdown_menu_wide, dropdown_row, icon, kept, theme,
};

/// The step a grip is dragging, while one is.
#[derive(Clone, Copy)]
struct Dragged(usize);

/// How near the edge of the view the pointer drags the page along, and how fast, points.
const SCROLL_EDGE: f32 = 48.0;
const SCROLL_STEP: f32 = 8.0;

/// What a card or a seam asked of the list, carried out once every card is drawn.
enum Change {
    Insert(usize, StepKind),
    Move(usize, usize),
    Shift(usize, bool),
    Remove(usize),
}

/// Where which steps of the recipe kept as `id` stand open is kept.
pub(super) fn folds(id: &str) -> Id {
    Id::new(("recipe-folds", id))
}

/// Opens the first `count` steps of the recipe kept as `id`, as presses on their headings would.
#[cfg(all(test, feature = "snapshots"))]
pub(crate) fn unfold(ctx: &egui::Context, id: &str, count: usize) {
    kept::set(ctx, folds(id), &vec![true; count]);
}

/// The chain of `recipe`'s steps, folded as the user left them under `folds`; `editable`
/// false only folds them.
pub(super) fn chain(
    ui: &mut Ui,
    recipe: &mut RecipeForm,
    (checked, names): (&Checked, &[ConditionName]),
    folds: Id,
    editable: bool,
) {
    let count = recipe.steps.len();
    let mut open: Vec<bool> = kept::get(ui.ctx(), folds).unwrap_or_default();
    open.resize(count, false);
    let dragging = DragAndDrop::payload::<Dragged>(ui.ctx())
        .map(|dragged| dragged.0)
        .filter(|at| *at < count);
    let mut seams = Vec::with_capacity(count + 1);
    let mut cards = Vec::with_capacity(count);
    let mut dropped = None;
    let (first, mut change) = seam(ui, 0, (false, count > 0), editable && dragging.is_none());
    seams.push(first);
    for (index, (step, open)) in recipe.steps.iter_mut().zip(&mut open).enumerate() {
        let state = Card {
            index,
            count,
            open: *open,
            dragged: dragging == Some(index),
            editable,
        };
        let drawn = card(ui, step, &state, (checked, names));
        if drawn.folded {
            *open = !*open;
        }
        dropped = dropped.or(drawn.dropped.then_some(index));
        change = change.or(drawn.change);
        cards.push(drawn.rect);
        let below = index + 1 < count;
        let (rect, asked) = seam(ui, index + 1, (true, below), editable && dragging.is_none());
        seams.push(rect);
        change = change.or(asked);
    }
    if let Some(from) = dragging.or(dropped) {
        let moved = drag(
            ui,
            recipe.steps[from].kind,
            from,
            (&cards, &seams),
            dropped.is_some(),
        );
        change = change.or(moved);
    }
    if let Some(change) = change {
        apply(recipe, &mut open, change);
    }
    kept::set(ui.ctx(), folds, &open);
}

/// Where a card stands in the chain and how it is shown.
struct Card {
    index: usize,
    count: usize,
    open: bool,
    dragged: bool,
    editable: bool,
}

/// What a card's heading and body asked, and where it stands.
struct Drawn {
    rect: Rect,
    folded: bool,
    dropped: bool,
    change: Option<Change>,
}

/// The card of `step`, its fields inside while open.
fn card(
    ui: &mut Ui,
    step: &mut StepForm,
    state: &Card,
    (checked, names): (&Checked, &[ConditionName]),
) -> Drawn {
    let index = state.index;
    let summary = summary(step);
    let parts: Vec<&str> = summary.iter().map(String::as_str).collect();
    let (name, help) = (kind_name(step.kind), step::help(step.kind));
    let grip_tooltip = tr!("recipes-step-drag");
    let faulty = checked.in_step(index);
    let (up, down, remove) = (
        tr!("recipes-step-up"),
        tr!("recipes-step-down"),
        tr!("recipes-step-remove"),
    );
    let actions = [
        (icon::MOVE_UP, up.as_str(), index > 0),
        (icon::MOVE_DOWN, down.as_str(), index + 1 < state.count),
        (icon::REMOVE, remove.as_str(), true),
    ];
    let look = ChainStep {
        number: index + 1,
        glyph: kind_glyph(step.kind),
        name: &name,
        summary: &parts,
        help: &help,
        grip_tooltip: &grip_tooltip,
        actions: &actions,
        faulty,
        // A step that does not read stands open, so its error is in sight.
        open: state.open || faulty,
        dragged: state.dragged,
        editable: state.editable,
    };
    let id = ui.id().with(("recipe-step", index));
    let mut on = step.enabled;
    let answer = chain_step(ui, id, &look, &mut on, |ui| {
        step::fields(ui, step, index, checked, names);
    });
    step.enabled = on;
    if answer.grip.dragged() {
        DragAndDrop::set_payload(ui.ctx(), Dragged(index));
    }
    Drawn {
        rect: answer.rect,
        folded: answer.folded,
        dropped: answer.grip.drag_stopped(),
        change: answer.action.and_then(|at| match at {
            0 => Some(Change::Shift(index, true)),
            1 => Some(Change::Shift(index, false)),
            2 => Some(Change::Remove(index)),
            _ => None,
        }),
    }
}

/// While step `from` of `kind` is dragged: the mark of the seam it would land at, the page
/// following the pointer and the card by it; once `dropped`, the move to that seam.
fn drag(
    ui: &Ui,
    kind: StepKind,
    from: usize,
    (cards, seams): (&[Rect], &[Rect]),
    dropped: bool,
) -> Option<Change> {
    let to = landing(ui, cards).filter(|to| *to != from && *to != from + 1);
    if let Some(to) = to {
        chain_drop_mark(ui, seams[to]);
    }
    if dropped {
        return to.map(|to| Change::Move(from, to));
    }
    follow_pointer(ui);
    chain_ghost(ui.ctx(), kind_glyph(kind), &kind_name(kind));
    None
}

/// The seam at `at`, its + listing every kind of step; answers the one chosen to stand there.
fn seam(ui: &mut Ui, at: usize, ends: (bool, bool), add: bool) -> (Rect, Option<Change>) {
    ui.push_id(("recipe-step-add", at), |ui| {
        let (rect, pressed) = chain_seam(ui, &tr!("recipes-step-add"), ends, add);
        let chosen = pressed.and_then(|pressed| {
            dropdown_menu_wide(&pressed, theme::BLOCK_TITLE_W, |ui| {
                StepKind::ALL.into_iter().find(|kind| {
                    let name = kind_name(*kind);
                    dropdown_row(ui, &DropdownRow::named(&name, false)).clicked()
                })
            })
            .flatten()
        });
        (rect, chosen.map(|kind| Change::Insert(at, kind)))
    })
    .inner
}

/// The seam the pointer stands at among `cards`: the count of cards whose middle is above it.
fn landing(ui: &Ui, cards: &[Rect]) -> Option<usize> {
    let pointer = ui.ctx().pointer_interact_pos()?;
    Some(
        cards
            .iter()
            .filter(|card| card.center().y < pointer.y)
            .count(),
    )
}

/// Scrolls the page while a dragged card is held near the top or the foot of the view.
fn follow_pointer(ui: &Ui) {
    let Some(pointer) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let view = ui.clip_rect();
    let delta = if pointer.y < view.top() + SCROLL_EDGE {
        SCROLL_STEP
    } else if pointer.y > view.bottom() - SCROLL_EDGE {
        -SCROLL_STEP
    } else {
        return;
    };
    ui.scroll_with_delta(vec2(0.0, delta));
    ui.ctx().request_repaint();
}

/// Carries `change` out on the steps, and on which of them are open, so each fold follows
/// its step; a step just added opens.
fn apply(recipe: &mut RecipeForm, open: &mut Vec<bool>, change: Change) {
    match change {
        Change::Insert(at, kind) => {
            recipe.insert_step(at, kind);
            open.insert(at.min(open.len()), true);
        }
        Change::Move(from, to) => {
            recipe.move_step(from, to);
            let fold = open.remove(from);
            open.insert(if to > from { to - 1 } else { to }, fold);
        }
        Change::Shift(index, up) => {
            let to = if up {
                index.checked_sub(1)
            } else {
                Some(index + 1)
            };
            if let Some(to) = to.filter(|to| *to < open.len()) {
                open.swap(index, to);
            }
            recipe.shift(index, up);
        }
        Change::Remove(index) => {
            recipe.steps.remove(index);
            open.remove(index);
        }
    }
}

/// What a folded card says of its step: off, or its condition, a failure skipped, how many
/// settings and rules it holds.
fn summary(step: &StepForm) -> Vec<String> {
    if !step.enabled {
        return vec![tr!("recipes-summary-off")];
    }
    let when = step.when.trim();
    let counted = [
        (!when.is_empty()).then(|| tr!("recipes-summary-when", condition = when)),
        (step.on_fail == OnFail::Skip).then(|| tr!("recipes-summary-skips")),
        (!step.set.is_empty()).then(|| tr!("recipes-summary-set", count = step.set.len())),
        (!step.choose.is_empty()).then(|| tr!("recipes-summary-rules", count = step.choose.len())),
    ];
    counted.into_iter().flatten().collect()
}

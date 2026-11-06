# 0406. Stand Prepare for print over the plate card as a recipe and a run

- **Status:** Proposed
- **Date:** 2026-11-06

## Context

Prepare for print ran from a section at the foot of Select (ADR 0402, 0403): the recipe, the
button, the plan of each step and what the last run did, under Move, Rotate, Scale, Place and
Auto-orient. Reaching it meant taking up Select and scrolling past the transform, and the
plan made Select the longest card. It acts on the whole plate as readily as on the picked
model, which Select's other sections do not, and it is the step before Slice, which stands on
the plate card.

## Decision

Prepare for print is a row over the plate card, as wide, in Prepare while the plate holds a
model: the recipe picker and a square that runs it, bare on the stage, the list dropping as
wide as the row. While a run goes the row is a card holding the run's step bar with Cancel.
Select keeps the transform and Auto-orient; the Edit menu runs Prepare for print as before.

The window no longer shows the plan before a run nor what a run decided for a model: what a
step will do is the recipe's, read in Settings → Recipes, and what it did is on the plate and
under one undo.

## Consequences

Running Prepare for print is one press from any tool, beside Slice, in the order the two are
done, and Select is the transform alone. Why a recipe cannot run is read on the run square's
hover rather than under the picker. The row stands on the stage without a card, the one part
over the stage that does; a run puts it on one, so its step reads over any model. Bringing
the plan back, or step switches a run takes without editing its recipe, is a new decision.

## Alternatives considered

### A card of its own in the inspector, opened from the row

Kept the plan and the last run's readings a press away, but the row with its run is what
Prepare for print is used through, and a card holding a plan that runs by itself was a
second place to read what the recipe already says.

### A tenth tool on the tool row, a bar under it, or a button beside Select

A tool means taking it up before running; the bar takes a line over the stage and moves the
hint bar; the button hides the recipe until opened. None stands by Slice.

### The bare row, and what it costs

The plan before a run is gone from the window, and the row breaks the rule that whatever
stands over the stage is a card.

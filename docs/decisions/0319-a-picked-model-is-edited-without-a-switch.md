# 0319. Edit the picked model's Hollow and Drain numbers without a switch

- **Status:** Accepted
- **Date:** 2026-10-20

## Context

ADR 0227 lets a model keep its own Hollow and Drain numbers behind an "Applies to: Plate /
Model" switch. In use the switch is one more thing to set before every change, and a change
made with it left on *Plate* lands on every model that has none of its own, which is rarely
what a click on one model meant. Coming back to a model should show the numbers it was last
given.

## Decision

The switch is gone. With exactly one model picked, the Hollow and Drain panels edit that
model's numbers: one with its own shows them, one without shows the plate's and takes them
up as its own on the first change (`scope::editing`). With none or several picked, the
panels edit the plate's, which stay what a model without its own is run with. With none
picked, a line under the presets says how many models keep their own.

Everything else in ADR 0227 stands: how a run reads them, how the project stores them, undo.

## Consequences

A change made with one model picked never reaches another model, and reopening the tool on
that model shows its last numbers. Picking a model without changing anything leaves it
following the plate. There is no longer a way to drop a model's own numbers back to the
plate's from the panel; a preset applied to it sets them all at once.

## Alternatives considered

### Keep the switch, defaulting to Model

Still a control for something the selection already says, and a picked model would take a
copy of the plate's on sight rather than on a change.

### Edit the picked model's and the plate's at once

The plate's would then drift with whichever model was edited last, and a model without its
own would change behind the user's back.

### No switch, and what it costs

Setting the plate's numbers takes clearing the selection first, and a model's own numbers
cannot be thrown away short of applying a preset over them.

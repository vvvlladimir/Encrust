# 0470. A recipe's plate step is an edit

- **Status:** Proposed
- **Date:** 2026-11-19

## Context

ADR 0466 makes every change to a plate an `Edit` carried out by one `apply`. A recipe's steps
(`recipes.md`) still reached the plate their own way: the window's prepare read each step's
keys into its own jobs, the command line into its flags, each with its own defaults. Orient
was left out of the edits (plan card C8) because a client's pose is checked by a machine and
handed over as a turn; a recipe's orient step has no check to choose for it, so it had no edit
to become.

## Decision

`Edit::Orient` stands a model by the search on the object as it stands, with no machine's trial,
the `pick`th of every ranked pose. `core-recipe` depends on `core-edit` and answers a plate
step — repair, orient, hollow, drain, supports, arrange — with its edits through `edits_of`;
check, save, gate and pause, and arrange's slide-clear, are no edit (`NotAnEdit`). The window's
prepare reads its step settings through it; the command line's recipe executor keeps its own
reading until it runs on a `Document`. The goal words live once, in `core-edit`'s `OrientAim`,
and the word `auto` for a support profile once, in `core-edit`.

## Consequences

A recipe's step, a client's request and a hand edit mean the same plate change. The window's
prepare still runs its own batched jobs, one job over many models; moving it onto the edits
themselves needs batched edits and is open, as is running the command line's recipe executor
on a `Document`, after which a recipe's keys have one reading and one set of defaults. Reopen
if a step needs a setting no edit carries.

## Alternatives considered

### Keep Orient out of the edits and give a recipe's orient step a machine

A recipe would then need a printer and resin to orient, which a plate edited with no machine
does not have.

### The option that won, and what it costs

`core-recipe` builds `core-edit`, and an orient edit with no trial can pick another pose than a
client's check proved; a client's check still hands over its own turn.

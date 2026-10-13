# 0246. Keep an edited support profile on the picked models until it is saved

- **Status:** Accepted; what choosing another profile from the list does is superseded by 0247
- **Date:** 2026-10-13

## Context

A support is built to its group's profile and the table of groups is the plate's
(ADR 0094), so changing a number in the Shape panel rebuilt every model on the plate at
once. Two models built to one saved profile could not be tuned apart without giving one a
group of its own, which is what ADR 0227 pointed at when it left supports out of a
model's own numbers. Turning a raft on for one model turned it on for all of them.

## Decision

A model may keep a profile of its own per group: `OwnSettings::supports`, by group number.
Where it has one, everything built for that model — its trees, its raft and its lift, a
placement run, a fill — reads it in place of the plate's (`SupportTool::table_for`).

The Shape panel and the Supports tool's own numbers edit the picked models: an edit, or
another profile chosen from the list, is kept on them alone. With nothing picked they edit
the plate's, as a tool with nothing picked always has (ADR 0101). **Save** writes the
profile over the saved one of its name, the plate's numbers of that name become the saved
ones, and a model whose own are now no different lets go of them — which is when every
other model built to that profile follows. A model keeping different edits keeps them.

The project writes them on each `ObjectState` as `own_supports`, left out when empty and
read as empty from an older file, so the manifest stays version 4. `open_plate` builds a
model to `ObjectState::support_table`. Undo sees them through the scene's fingerprint.

## Consequences

Two models on one saved profile are tuned apart by picking one and typing. An unsaved edit
is no longer the plate's, so a model put down later is built to the saved profile, not to
what another model is being tried with. Every place that reads the tool's profile for a
model has to ask for that model's; one that reads the tool's directly is a bug. There is
no switch saying a model keeps its own: the profile picker reads "(edited)" for it, and
that is the only sign. Reopen this if a model's own profile is lost track of on a full
plate.

## Alternatives considered

### The Plate / Model switch of ADR 0227

Explicit, and one more thing to press before every edit; the common case is tuning the
model that is picked.

### A group per model

Already possible, but a group is a shape shared across the plate, and two models on one
saved profile are one shape until somebody edits it.

### The option that won, and what it costs

What an edit applies to now depends on what is picked, with nothing on screen but the
selection saying so.

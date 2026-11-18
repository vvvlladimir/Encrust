# 0397. The size the next click puts down is the window's, not a model's

- **Status:** Proposed
- **Date:** 2026-11-04

## Context

ADR 0227 lets a model keep its own Hollow and Drain numbers, and ADR 0319 edits them whenever
one model is picked. Both tools mix two kinds of number: what shapes the model — its wall,
infill, plugs' gap — and what only sizes the next click — a blocker's size or fit (ADR 0396),
a drain hole's diameter, depth, taper, outline and turn. A blocker or a hole already placed
carries its own size, so the second kind says nothing about the model. Kept on the picked
model, it was lost with it: a user who chose Fixed size with a model picked found Fit to
thickness again on the next run.

## Decision

- What sizes the next click belongs to the window. `HollowTool::blocker` is a `BlockerSize`
  out of `HollowState`, remembered in `ToolSettings::keep_solid`; the Drain tool's hole sizes
  stay in `DrainState`, which the window remembers, and `DrainTool::size` drills every model
  with them.
- A model keeps of the Drain tool only its plugs' gap: `OwnSettings::plug_gap_mm`, written as
  `own_drain: { plug_gap_mm }`. An older file's own hole sizes are read past.
- Opening a project changes neither; it brings the plate's plugs' gap. A Hollow preset no
  longer holds a blocker size; a Drain preset still holds a hole's, and picking it sets the
  window's.

## Consequences

The next blocker and hole are what the user last chose, whichever model is picked and after a
restart. A project or a model no longer carries a blocker size; an older file's is ignored.
A project still writes the plate's hole sizes under `drain`, as presets read them, but
opening it does not apply them. Reopen this if a model ever needs a hole size of its own,
as a recess it is printed with.

## Alternatives considered

### Keep the numbers on the model and remember the last one as well

Keeps a choice per model that nothing reads: a placed blocker or hole already has its size.

### Split `HollowState` and `DrainState` into a model part and a window part

Leaves the file layouts as they are with `serde(flatten)`, but moves every wall and hole
field one level down in a hundred places for two fields in Hollow and one in Drain.

### The option that won, and what it costs

A Hollow preset loses its blocker column, and a project's `drain` sizes are written but not
read back; the plugs' gap alone travels with a model.

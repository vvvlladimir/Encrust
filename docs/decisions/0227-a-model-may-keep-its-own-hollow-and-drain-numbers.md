# 0227. Let a model keep its own Hollow and Drain numbers

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

The Hollow and Drain tools hold one set of numbers for the plate (ADR 0192), and a run
works on the selection, or the plate when nothing is picked (ADR 0101). A plate of a thin
miniature and a thick bust wants two walls, and hollowing them one at a time with the
numbers retyped in between is the only way to get them; the next plate-wide run overwrites
one of the two. The mockup puts an "Applies to: Plate / Model" switch on these tools
(ADR 0218).

## Decision

A model may carry `OwnSettings { hollow: Option<HollowState>, drain: Option<DrainState> }`.

- A run, a stale check and every hole drilled read a model's own numbers where it has
  them and the plate's where it has not (`HollowTool::settings_for`,
  `DrainTool::size_for`).
- The switch is enabled with exactly one model picked. *Model* copies the plate's numbers
  onto it and the panel edits that copy; *Plate* drops the copy. Under it the panel says how
  many models keep their own.
- The project writes them on each `ObjectState` as `own_hollow` and `own_drain`, left out
  when absent and read as absent from an older file, so the manifest stays version 4.
- Undo sees them: the scene's fingerprint hashes them.

Supports and their shape are not scoped this way: a support is built to a group's profile
already (ADR 0094), which is a model's own shape when the model has a group of its own. The
print settings are not either: a layer is exposed once for everything on it.

## Consequences

Two models on one plate hollow to two walls in one run, and the plate's numbers stay what
the next new model takes. A model that kept its own is not changed by editing the plate's,
which can surprise; the count under the switch is what says so.

## Alternatives considered

### The switch meaning "this run, on the selection only"

Already what a tool does with something picked (ADR 0101); a switch for it adds a second
way to say the same thing and keeps nothing.

### Per-model overrides of every tool, print settings included

Exposure and layer height cannot differ within one layer of an MSLA print.

### Own numbers on the model, and what it costs

Every place that reads a tool's numbers for a model has to ask the model first; one that
reads the tool's directly is now a bug.

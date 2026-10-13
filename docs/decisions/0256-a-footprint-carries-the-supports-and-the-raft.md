# 0256. Pack a model with its supports and its raft

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

Arranging packs each model's own shadow on the plate (ADR 0088) and leaves its supports to
the clearance. That was a bet on supports staying under their model. They do not: a pad is
7 to 10 mm across on every shipped preset and stands half outside a contact near the rim,
a branch leans out by its merge distance, and a raft is drawn a margin wider than the part.
Arranging a supported plate set pads and rafts into the neighbouring model.

## Decision

**A footprint is the model with the supports and the raft last built for it.** The app
hands `core_plate::Footprint::of_parts` the model in its placement and every support mesh
of the model in plate coordinates, and they are rasterised into one mask, so they are
moved as one. A model with nothing built under it packs as it did.

## Consequences

- A supported plate arranges with no pad, trunk or raft under another model.
- A supported model takes more of the plate than its shadow, so fewer fit.
- The supports packed are the ones on screen: a plate arranged while a rebuild is pending
  packs the previous one, which differs only by what the last edit moved.
- The command line spreads models before it supports them, so it still packs each model
  alone and leaves its pads to the clearance.

Reopen if a support rebuilt after arranging leans further than the one packed, for
example because the model now stands over another's raft.

## Alternatives considered

### Grow the clearance by the pad's radius

No new code. Rejected because a raft's margin and a branch's lean are not the pad's, and a
plate with no supports would lose the room for nothing.

### Pack the model alone, then nudge whatever overlaps

Keeps the bitmap of ADR 0088 small. Rejected because a nudge undoes the packing it
follows, and the mask is cheap to grow by a few meshes.

### The option that won, and what it costs

Every support triangle is rasterised once per arrange, which for a densely supported plate
is as many triangles again as the models carry.

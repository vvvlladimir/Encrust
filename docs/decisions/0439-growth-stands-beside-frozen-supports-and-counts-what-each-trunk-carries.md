# 0439. Growth stands beside frozen supports, counts what each trunk carries, and frozen supports follow their model

- **Status:** Proposed
- **Date:** 2026-11-12

## Context

A frozen support (ADR 0095) is kept in the model's own space and meshed beside the grown
ones, but growth knew nothing of it:

- A thin tip beside a frozen thick column could not run into it (ADR 0248), as it would into a
  grown one. A small pillar grown again by a carry (ADR 0438) stood on nothing but the part or
  the plate, never on a support beside it.
- `max_tips_per_trunk` was counted only within one family's merge. The next family began at
  nought on the same carriers, and neither small pillars run into supports (ADR 0251) nor
  rescued columns (ADR 0429) counted at all: on a column allowed two tips, three small pillars
  stood.
- Moved, the model carried its frozen supports with it. Raised by a lift, their feet stood in
  the air; tilted, their trunks leaned and their feet left the plate.

## Decision

- **`grow_beside` grows beside frozen supports.** Their struts and trunks are carriers before
  the first family grows, so a thinner front runs into them as into a thicker group's; they are
  never grown again. The model's rebuild and its growth on a worker both hand them in, and a
  small pillar grown again by a carry stands beside every other frozen support. A frozen support
  removed, cut or taken away by its group grows the rest again, so nothing grown stays on it.
- **What a trunk carries is counted once, across the whole growth.** `Carriers` counts the tips
  of every tree it takes on standing on an earlier one; each family's merge starts from that,
  small pillars are seated one at a time, highest seat with room first, and a rescue running
  into a support filled by those before it is grown once more.
- **Frozen supports follow their model.** `ModelSupports` keeps the placement they last stood
  at. Moved across or up and down, or turned about the plate's up, each foot on the plate stands
  on it again — the trunk lengthened or shortened, refused under a pillar's height; tilted or
  resized, every frozen support goes back to growth, its tips kept as points.

## Consequences

A support grown after a hand edit may stand on what the hand made, and is grown again if that
goes. No trunk carries more tips than its group allows, which may leave a few more small
pillars on the part. Raising a model keeps its hand-made supports on the plate; tilting it
loses their shape and keeps their contacts. A small pillar seated on a support costs one
soundness check per support in reach rather than one; growth's numbers are in
`cargo bench -p core-supports --bench branch`. Reopen if hand-made shapes lost to a tilt
matter more than a trunk that leans with it.

## Alternatives considered

### Keep frozen supports out of growth

What was there: simple, and grown and frozen never depend on each other, but a thin tip beside
a hand-placed trunk stands a trunk of its own.

### Lean a frozen support with its tilted model and refuse it

Keeps the shape, but leaves a support that slicing prints leaning off the plate.

### Growing beside them, and what it costs

A removed frozen support regrows the model's supports on a worker, about a second for a large
model, where before it only meshed again.

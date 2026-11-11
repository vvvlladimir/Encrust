# 0433. A support standing on another goes where that one is carried

- **Status:** Proposed
- **Date:** 2026-11-11

## Context

Growth stands one support on another: a thinner front ends on the trunk or a strut of a
thicker tree (ADR 0248), and a small pillar runs into a wider support beside it (ADR 0251).
The landing keeps only where it meets, `on_support` set, and names no tree. Taking hold of
the carrying support froze it alone (ADR 0095), and carrying it left everything standing on
it where it was: branches ending in the air, or in a trunk that had gone.

## Decision

- **Taking hold of a support freezes what stands on it with it**, directly or through
  another: `ModelSupports::freeze_with_riders`, the riders found by `standing_on`.
- **The link is found, not stored.** A support stands on another when its landing lies on
  one of that tree's axes, within 0.01 mm: the struts a front may meet and the trunk, as
  `branch::carriers` offers them. It is kept as which axis and how far down it, as a share.
- **A carried support takes them along.** `followed` puts each at the same share of the same
  axis of the carried tree, then those standing on it in turn. Only its landing moves; its
  tips and joints stay, so its trunk bends to meet the new seat.
- **A step that sinks one of them is refused whole**, as a step sinking the carried support
  is (ADR 0095). One that did not fit before the step does not hold the step back.

## Consequences

Carrying a trunk, its foot, a joint or a tip keeps every branch on it attached. A touch
freezes more than was touched, so those supports no longer regrow with the rest; **Grow these
again** hands them back as before. A support taken away still leaves what stood on it in the
air. The link is lost if an axis moves more than 0.01 mm outside a drag.

## Alternatives considered

### Store the carrying tree in the landing

Survives any edit, but it is a file-format change, and every edit that renumbers trees
(freeze, remove, regroup, regrow) would have to keep it true.

### Carry the riders without freezing them

A rebuild regrows a grown tree from its points and forgets where it was carried.

### Finding the link by geometry, and what it costs

Each step looks over the model's frozen trees for each one carried: linear in frozen trees,
which a hand made one at a time.

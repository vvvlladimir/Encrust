# 0423. Build every automatic tip lightest, and size it by what it carries

- **Status:** Superseded by 0424
- **Date:** 2026-11-11

## Context

An automatic profile picks a support's shape by a measure of what it holds: the area of the
island it begins, or the model standing over the overhang it is under (ADR 0325). Since
ADR 0422 every automatic placement also reads what the film pulls on each tip and builds a
tip under ×1.5 one size heavier. On the bench's models nearly every tip of 0.40 mm the area
and volume rules chose holds with ×20 to ×100 to spare (998 on one figure, the least ×20):
the rules make tips thick, not their load. A thick tip scars the part and costs resin.

## Decision

- **Every automatic tip starts at the profile's lightest shape.** `Need::size` is the
  lightest plus the sizes strengthening added; a steadying support keeps its own shape. The
  `Reason` a contact is wanted for still comes back with its measure, for the bench, but
  sizes nothing.
- **Strengthening alone makes a tip heavier**, as ADR 0422 has it.
- **`overhang_volume_mm3` leaves the automatic profile**, with its rows in the window. An
  older file still reads; the field is ignored. `island_area_mm2` stays, as the areas an
  island is spaced wider from; `step_volume_mm3` stays, as the sizes of a step's reach.
- **The window loses its Sizes card and block.** The island areas move under Placing, the
  step thresholds under Steps, each row named by its step rather than by a shape.

## Consequences

On the bench (`--min-tip-mm 0.3`, against the area and volume rules) nearly every tip is
0.30 mm; only three on the thin-part model are thickened, as before. The area of the marks
falls by 25 to 45 %, resin by 5 to 20 %; no tip keeps under ×1.5, the least margin falls
from ×22 to ×9.5, and no verdict or island changes. One model takes 8 % more resin, its
lighter shape braced differently.

The safety of a tip now rests on the load read alone: a tip the read calls safe and that
tears is the signal to reopen this, as is a second resin or layer height.

## Alternatives considered

### Keep the rules as a floor

A tip at least as heavy as its area or volume asks, heavier where its load asks. Closer to
ADR 0325, but the floor is what made the tips thick.

### More shapes, or a diameter on every tip

Five or six shapes as groups, or a width per support in `.encrust`. Finer, but nothing yet
shows three shapes are too coarse once the load chooses among them.

### The option that won, and what it costs

A tip is light unless the read says otherwise: an error in the read that ADR 0422's three
rounds do not catch is a torn tip where the old rules would have built a heavy one by
accident. No print has yet set a miniature placed this way beside one placed by the rules.

# 0415. Check's buttons answer what it finds, through the calls the command line makes

- **Status:** Accepted; its cups left to the hand superseded by 0417, its arrange by 0418
- **Date:** 2026-11-09

## Context

Check named every issue but could fix only islands; the rest were a jump to a layer or a tool
to open. Two answers did not exist. A suction cup had no way to become a hole: a `Risk` holds
only a layer and a point on the panel, and Drain's scan looks for trapped pockets, not for
cups. A weak neck had no way to become a support, since `island_holds` looks only a layer or
two above an island's floor. Arrange moved every model and ignored a printer's plate inset, so
it could put a model back where Check had just flagged it.

## Decision

A cup is vented by a hole up through its floor and a neck is held by a support under the piece
pulling on it, each found by a straight ray in `core_engine::held` (`docs/design/check-fixes.md`).
`core_plate::move_clear` moves only what is in the way by `nearest_free`, on bounding boxes, and
`arrange` packs into the `PlateArea` an inset leaves. A recipe reaches them by `check`'s
`drill_cups` and `hold_levers`, once before its retry, and by `arrange`'s `only_in_the_way`.

## Consequences

A cup or a neck is one press, one undo, and the same on the command line. `--arrange` no longer
packs into a printer's inset, so a plate that only fitted across it now reports no room. A cup on the
plate and a ring-shaped cup whose centre is solid are left to the hand and counted in the
notice. A drilled floor marks the model: Auto-orient remains the answer where a hole would
show. A vent that often misses its cup, or necks still named after their support, is the
signal to reopen this.

## Alternatives considered

### Vent a cup by a side channel

It would serve a cup standing on the plate, but it routes a tube through the model, and a new
search would have to prove it drains.

### Reuse the trapped-pocket scan for cups

Its proof is that a pocket of trapped resin empties, and a cup open toward the film has no
trapped resin to prove it by.

### Nest the moved model on its footprint mask

It would use concave room, at the cost of a second search beside `nearest_free`; Arrange all
already nests.

### The option that won, and what it costs

A single straight ray can miss: it does not look for the best spot on a floor or an underside,
and one support is not a calculated answer to the pull on a neck.

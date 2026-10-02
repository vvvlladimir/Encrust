# 0085. Price a build by its larger phase, not by one constant a tile

- **Status:** Accepted
- **Date:** 2026-09-24

## Context

ADR 0063 refuses a field the machine cannot hold, by counting the tiles a build will fill
before filling any and pricing them at `TILE_COST_BYTES` — one constant, measured at 7.5 kB
against the gather that then filled them.

ADR 0082 replaced that fill. A build now has two phases that do not coexist: filling holds
the coarse carrier, which is swept across the wall and dropped; meshing holds the cavity,
which is not built until the carrier is gone. `hollow-lab` on a 60 mm ball at 0.05 mm, over
three walls:

| wall | stored tiles | carrier tiles | filling | meshing |
|---|---|---|---|---|
| 0.5 mm | 137 879 | 65 327 | 394 MB | 447 MB |
| 2 mm | 124 265 | 92 924 | 500 MB | 430 MB |
| 6 mm | 90 884 | 152 908 | 750 MB | 397 MB |

No constant a stored tile fits those: the ratio runs from 3.2 kB to 9.2 kB, because what
moves is the carrier, and the carrier follows the wall. Left at 7.5 kB the budget
over-prices a thin wall by half and under-prices a thick one by a fifth — it coarsens runs
that would have fit, and clears runs that will not.

## Decision

`affordable` prices the larger of the two phases rather than a single number a tile:

- filling: `stored × 1 kB + carried × CARRIER_TILE_BYTES`, the stored tiles' own values
  and the carrier beside them
- meshing: `stored × TILE_COST_BYTES`, the tile plus the triangles cut from it

`TILE_COST_BYTES` drops to 3.7 kB, which is what extraction now costs a tile, and
`CARRIER_TILE_BYTES` is 4.3 kB — a carrier tile's own five hundred faces plus the two rings
kept beside it as halo. Both counts are known before a voxel is filled: the stored tiles
from the walk, the carrier's from `sweep::carrier_tiles` over the same rings.

This keeps what ADR 0063 was for — a run ends in a coarser cavity, never in an exhausted
machine — and changes only the arithmetic it refuses on.

## Consequences

The model predicts 510, 527 and 751 MB against the 447, 500 and 750 measured, so it is
honest to about a seventh and errs high, which is the safe side of a memory ceiling.

`VolumeError::TooFine::fits_at_mm` is weaker than it was. It scales the spacing by the root
of how far over budget the run is, which assumed the count followed an area over the
spacing squared. The carrier's does not: it follows the shell's volume over the spacing
cubed, so a coarsening step can undershoot where the carrier dominates. `hollow` retries up
to three times and each retry costs a walk rather than a run, so this shows as a slower
refusal and never as a wrong one. The signal to reopen: a run that takes all three
coarsenings on a machine that is not busy.

Both constants are measurements of one build on one shape. `hollow-lab` is what re-takes
them, and any change to what a tile or a carrier tile holds invalidates them silently.

## Alternatives considered

### Keep one constant, set to the worst wall measured

`TILE_COST_BYTES` at 9.5 kB covers the 6 mm wall and never under-prices. Rejected because
it then over-prices a thin wall threefold, coarsening cavities that had room — which is
exactly what ADR 0069 was written to stop doing.

### Sum the two phases instead of taking the larger

Simpler to state and never optimistic. Rejected on the numbers: it predicts 771 MB where
447 MB was measured, so a third of the budget would go to a peak that never happens.

### The decision above, and what it costs

Two constants where there was one, and a rule — "the larger of" — that is only true while
the carrier is genuinely dropped before the cavity is meshed. Nothing enforces that
ordering; it is a property of how `fill` is written today, and a change that held the
carrier a little longer would make this budget quietly optimistic with no test failing.

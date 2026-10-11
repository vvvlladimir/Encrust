# 0224. Read suction cups and thin sections off the stack

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

ADR 0164 reads islands, levers and peels off the stack a layer at a time. Two more ways a
resin print fails were left to the user's eye: a hollow sealed against the film side, which
every lift pulls a vacuum in, and a feature too thin to survive the peel. The Check tool
lists both (ADR 0218). They have to fold into the same pass: the stack is never held whole.

## Decision

`Measured` folds two more readings into `push`, each against the layer below alone:

- **`RiskKind::SuctionCup { area_mm2, volume_mm3 }`**: a hole sealed from below, traced up
  the stack and named once where it is widest, past `MIN_CUP_MM2` (1 mm²). Holes are found
  from the dark runs between lit ones, so the cost follows the runs, not the pixels.
- **`RiskKind::Thin { width_mm }`**: a held piece narrower than `THIN_MM` (0.15 mm) as a
  disc of its area, named once at the start of a run.

The rules are in `docs/design/analysis.md`. `worst` ranks them after levers and before
peels; the batch report counts both.

## Consequences

The window can say "drill this" before a print vacuums itself off the plate. Both are
judged from masks and know nothing of supports: a 0.15 mm limit keeps tips out only while
no profile puts a tip under it, and a cup over a narrow vent that is open still reads as
vented. Holes are joined on four neighbours while pieces join as `cure` does; a diagonal
gap can split one hole into two cups. Reopen the thin limit if users report tips named.

## Alternatives considered

### Width from a distance transform

The true narrowest width of every piece, at the cost of a pass over every pixel of every
layer, against runs of a few thousand. The disc of the area is exact for round sections,
which is what a needle is.

### Cups from the drain check's air flood

`core-supports` already floods air through the stack for trapped resin, but closed pockets
only; a cup is open on the film side, which is the case it skips.

### One pass, and what it costs

Both readings only see the layer below. A cup vented by a hole that opens fifty layers
later is still named.

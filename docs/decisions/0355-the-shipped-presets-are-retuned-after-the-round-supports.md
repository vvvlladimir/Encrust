# 0355. Retune the shipped presets after the round supports

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

ADR 0323 made Light, Medium and Heavy differ in thickness only, ADR 0343 gave them flares of
2.0, 2.4 and 3.6 mm, and ADR 0347 shipped a longest strut of 6 mm everywhere. Every shipped
profile kept its own bracing (ADR 0329). Once supports were drawn round, filleted, leaned
and checked as drawn (ADR 0341–0352), prints of the shipped presets set the numbers again:
Heavy's 3 mm pillar marked more than it held, thin pads lifted off the plate on the peel,
and bracing that started at 3 to 8 mm left the lower trunks free.

## Decision

- **Heavy** has a 2 mm pillar, trunks of at most 3 mm, a 2.6 mm flare and holds surfaces
  leaning past 45 degrees; Light and Medium keep 40.
- **Medium's flare** is 3.0 mm, and every automatic shape's is 3.0 mm.
- **Every pad** is 0.5 mm thick, Light's and the automatic shapes' included.
- **Bracing** on Light, Medium, Heavy and Auto's medium shape is 0.9 mm across, at most
  10 mm apart, from 1 mm up, zigzag at the bottom and in the middle and crossed at the top,
  rising every 5 mm, Light's every 10. Medium, Auto's medium and Auto's heavy shape prop a
  free top with buttresses (pairs above 30 mm, one above 60); the others do not.
- **Auto's light and steadying shapes** merge struts up to 10 mm long; every other
  shipped profile keeps 6.
- **A file without `facets`** reads as 12 sides, as every shipped profile has (ADR 0352).

This supersedes the preset numbers and the one overhang angle of ADR 0323, the flare widths
of ADR 0343, the 6 mm shipped in every profile by ADR 0347, and the shipped bracing left as
it was by ADR 0329.

## Consequences

The presets no longer differ in thickness only: Heavy holds fewer surfaces than the other
two, and Light braces more sparsely. Auto's thinnest shapes reach further before a tip
gets a trunk of its own, so their trees are fewer and longer. A support profile saved by
the user is unchanged; one without `facets` is drawn with 12 sides instead of 24. Reopen
if Heavy at 45 degrees leaves sagging overhangs, or the 10 mm struts of Auto's light
shape wave on prints.

## Alternatives considered

### Keep the presets of ADR 0323

The same placement for every preset kept the automatic profile's sizing simple, but the
prints above showed the numbers did not hold up.

### A new preset beside the old ones

Two presets with almost the same numbers make the choice harder and double the tuning
(ADR 0339).

### The option that won, and what it costs

The presets no longer share one placement, so moving a model between them can change
which surfaces are held, and a test or benchmark measured against the old numbers measures
different supports.

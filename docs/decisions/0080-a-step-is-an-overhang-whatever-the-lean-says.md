# 0080. A step is an overhang, whatever the lean says

- **Status:** Accepted
- **Date:** 2026-09-23

## Context

ADR 0031 measures an overhang as ground covered over a millimetre of rise, because at
0.05 mm layers a 45 degree lean moves less than one cell of the grid and a single layer
cannot resolve an angle. Everything a layer gains is then compared against that reach.

A step defeats it. A model whose surface climbs in shelves — a stepped dome, a terraced
base, a staircase — puts a whole millimetre of shelf out in one layer, and the rule reads
it as a surface leaning 45 degrees over the rise, which the profile calls self-supporting.
Measured on two boxes with a 1 mm step between them: no supports at all under the upper
shelf. The viewport, whose wash is a face-angle test, marks that shelf red the whole time.

A lean and a shelf are not the same load. A lean is carried by the cured layer under it
all the way along; a shelf has nothing under it but film.

## Decision

An overhang is material that has moved further than the reach over the reference rise
**or** further than `min_overhang_mm` in this one layer. The second test is the same
`uncarried_by` walk against the layer below rather than the reference, at the width the
resin bridges — the width a ledge already has to exceed to be worth a support at all.

## Consequences

Stepped models get held where they are actually unsupported, and the window and the wash
agree about them.

Anything terraced costs more supports, and that includes a coarsely tessellated curve,
whose facets are shelves of exactly this kind. A sphere is only affected where its bands
are wider than `min_overhang_mm`; at the shipped preset that is 0.8 mm, so a fine mesh is
untouched and a very coarse one is treated as the staircase it is.

`min_overhang_mm` now carries two jobs: the smallest ledge worth a column, and the widest
step the resin bridges. They are the same number for the same reason — what a head can
carry is what the layer can span — but if one ever needs tuning apart from the other, this
is the ADR to supersede.

## Alternatives considered

### Scale the per-layer allowance with the profile's angle

`layer_height * tan(max_overhang_deg)` is the principled figure — 0.048 mm at the shipped
settings, which is half a cell. It would flag every band of every faceted model and most
of the staircase any sloped surface is printed as. The grid cannot tell those from a
shelf; the bridging width can.

### Leave placement alone and stop the wash marking staircases

Cheaper, and it would have made the window honest rather than right. It also leaves a
deliberate step — which is a real shelf on a real model — unheld.

### The option that won, and what it costs

One number now decides both what is too small to bother with and what is too wide to
bridge, so a profile that wants finer coverage of small ledges also gets stricter about
steps. And the test is still on the rasterised grid, so a step under a cell wide is
invisible to it however the model meant it.

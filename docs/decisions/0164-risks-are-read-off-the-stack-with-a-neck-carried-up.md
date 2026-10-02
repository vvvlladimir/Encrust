# 0164. Risks are read off the stack, each piece carrying the narrowest neck under it

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

Step 17b has to name the layer a print fails on. Islands are already found by
`core-supports`, but on its contour grid and before supports are meshed, so a support that
misses its contact is not seen. A lever needs more than one layer: what breaks is the
thinnest section somewhere below, pulled by a layer far above and off to one side of it.
The fold has to stay a single pass that holds one layer (ADR 0066).

## Decision

`core-analysis` joins each layer's pieces to the layer below by the pixels they share. A
piece that shares none is an **island**; one standing only on islands floats and is not
named again. Every anchored piece carries a **neck** — the narrowest section between it
and the plate, its area and where it is — as the lesser of its own contact and the necks
under it. Its layer's pull, Stefan's torsion constant times a fixed `PEEL_N_PER_MM4`,
stresses that neck in tension and in bending over the lever to the piece's centre; past
`NECK_LIMIT_MPA` it is a **lever**, named once per neck at its worst layer. A layer whose
whole pull passes `PEEL_LIMIT_N` is a **peel**, named once per run of such layers. Asked
to, the fold **removes islands** instead of naming them: the piece is erased from the
runs before they are encoded, and the next layer is judged against what is left, so a
floating part goes whole. Preview paints a layer's islands in the danger colour and
opens the issues in a view of their own.

## Consequences

Islands and levers come from what is printed, supports included. The three constants are
an order of magnitude, not a calibration: a lever and a peel are warnings. Removing an
island splits a write into rasterise, fold and encode, since a layer's islands depend on
the one below as it was written. A piece resting on several
necks shares their area as though they were one post, which is generous to a wide stance.

## Alternatives considered

### Islands from `core-supports`

Already there, but on contours before the supports exist, and only on a grid.

### Lever as a ratio with no units

Free of any constant, but a pull scaled only against its own neck flags every supported
flat part, since a slab always pulls harder than the tips it stands on.

### The option that won, and what it costs

Three constants nobody has measured, and a neck that only knows its narrowest point, not the
whole chain: a long thin support under a wide neck is judged by the neck.

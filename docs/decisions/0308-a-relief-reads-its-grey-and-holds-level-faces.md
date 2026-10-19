# 0308. Read a relief's grey and hold its level faces in the press

- **Status:** Accepted
- **Date:** 2026-10-18

## Context

A relief pressed into the field (ADR 0116, 0225) read grey one way only: white out, black
where it was. A picture drawn the other way round had to be edited outside first, a pressed
relief always added material, and a noisy photograph pressed its noise as steps of the
lattice. It also moved every face, so a model standing on its own base lost the flat
underside it stood on, and a lid lost the flat top a label or a mating part needed.

## Decision

`ReliefSettings` carries `invert`, `symmetric`, `smoothing_px` and an `AngleMask` of
`bottom_deg` and `top_deg` against an `up` given in the mesh's own space.
`core_volume::prepared` smooths the image with three wrapping box blurs standing in for a
Gaussian, then turns it over, and is what both the press and the window's drawing read.
`symmetric` subtracts mid grey from each height. A lattice point whose nearest held face is
as near as the nearest face of the model keeps its value; one outside the model past a held
face's edge is cut off at that face's plane. The window turns the plate's +Z into each
model's space through its rotation; the CLI presses after placement and passes +Z. See
`docs/design/volume.md`.

## Consequences

The window and `slice` both gain the four controls, and the default leaves a relief as it
was. A mask doubles the nearest-point queries where it is on. The cut at a held plane is
local, so a held face shows its relief stopping square at its edge rather than fading out,
and a model whose held faces see other surfaces within the band of each other may cut a
relief it should not; nothing seen so far does.

## Alternatives considered

### Inverting and blurring the picture in the window

Cheaper to write, but the CLI and a model's own texture would read grey differently from
the panel, and the drawing would not match the press.

### Fading the relief out near held faces

Softer, but a fade still moves a held face's edge and never gives the square bottom a base
or a mating face needs.

### The option that won, and what it costs

Four fields on a settings struct that every caller fills, a second hierarchy over the held
faces, and a clip rule that leans on the field being a true distance near the surface.

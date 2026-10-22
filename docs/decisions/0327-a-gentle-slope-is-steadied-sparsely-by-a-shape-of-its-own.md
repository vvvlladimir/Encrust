# 0327. Steady a part sparsely on its gentle slopes, with a shape of its own

- **Status:** Accepted
- **Date:** 2026-10-22

## Context

A face leaning less than the overhang angle holds itself up, so placement leaves it
alone. A part that rises on such faces — a statue's robe, a sloped base — still hangs on
the plate by whatever holds its overhangs, and the peel rocks it: nothing under the
broad, gentle underside keeps it from swinging. The supports that steady a part held at
too few places (`Reason::Steady`) have so far been built to the lightest size (ADR 0325).

## Decision

A run sizing its supports also asks `unsupported` for the faces leaning past
`steady_lean_deg` (10 degrees from vertical) and short of a step's angle, leaving out a
shelf's foot as it does for gentle faces (ADR 0326). A slope piece across less than the
side of `steady_area_mm2` (3.4 mm²) is left alone. Slopes are sampled at
`steady_spacing_mm` (7.9 mm), last in the layer, and a gentle face no step reaches joins
them. Each support so put down keeps the next one a coverage of that spacing away, in a
set of its own (`Zones` in `step.rs`), so it keeps away only its own kind: an overhang
above it is still held at its own spacing. They come back as `Reason::Steady`.

The automatic profile names a fourth shape, `steady_shape`, which every `Reason::Steady`
is built to — the ones under a part held at too few places too. It ships as
`auto-stable`: a 0.18 mm contact on the same 1.2 mm pillar, and a 4.5 mm pad.

## Consequences

Parts with broad gentle undersides get a few thin supports there, in a group of their own
the notice counts apart, which merge into the trunks beside them like any other of their
width. The area test is on one layer's piece, not on the face as a whole, so a narrow
slope tall enough to matter is skipped; reopen this if such a part is seen to rock. Sizing
costs another reach per layer and a look into a second set of buckets per sample: the
`tables` benchmark stays about 6 % over a run that does not size.

## Alternatives considered

### Steady every slope at the regular spacing

No new numbers, but a gentle face would carry as many supports as an overhang, scarring a
surface that needs only a touch.

### Let a steadying support keep everything away at its spacing

One set of supports, but a steadying support low on a part would then keep the overhangs
for 7 mm above it unheld.

### The option that won, and what it costs

A fourth group on a sized plate, a second set of reaches kept through the run, and a
measure of a slope's size that is only an approximation of its area.

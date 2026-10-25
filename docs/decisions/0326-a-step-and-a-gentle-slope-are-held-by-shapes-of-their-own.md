# 0326. Size a step by the resin it puts out, and steady a part sparsely on its gentle slopes

- **Status:** Accepted
- **Date:** 2026-10-22

## Context

A step — material put out further than the resin bridges in one layer — is held as an
overhang (ADR 0080), and an automatic profile sizes an overhang by the model above it
(ADR 0325). That misses what a step is: a shelf cured onto film in one exposure, whose pull
is its own area, however little stands on it. The face that rises from the edge of a shelf
leans out over a hold only one layer old, and at a lean the overhang angle lets pass on a
cured wall it sags off the shelf before the shelf has set. And a face leaning less than the
overhang angle is left alone, so a part that rises on such faces — a statue's robe, a sloped
base — hangs on whatever holds its overhangs, and the peel rocks it.

## Decision

Both apply to a run that sizes its supports; a run built to one group, with no automatic
profile, finds and holds steps as before and steadies nothing.

**Steps.** `unsupported` splits the shelf put out over the layer below from the lean
measured over the rise, and answers the faces leaning past `step_lean_deg` (20 degrees from
vertical) but not past the profile's angle, leaving out the foot of a shelf that its wall
carries. A step's size is the resin its piece puts out in that layer, area times layer
height. The automatic profile gives the least that counts as a step at all and where the
second and third shape begin — 0.1, 7 and 12 mm³ — and how far above a step of each size its
reach goes, 1.5, 2.4 and 3.2 mm. A smaller step is the overhang it was. Each sample of a step
is kept, while the climb is within its reach, as a ball of that radius above it (`Zones` in
`step.rs`, bucketed by the tallest reach, dropped once passed). An overhang or a gentle face
inside such a ball is held as a step of the largest one reaching it, `Reason::Step` with its
resin.

**Slopes.** `unsupported` also answers the faces leaning past `steady_lean_deg` (10 degrees)
and short of a step's angle, the shelf's foot left out likewise. A slope piece across less
than the side of `steady_area_mm2` (3.4 mm²) is left alone. Slopes are sampled at
`steady_spacing_mm` (7.9 mm), last in the layer, and a gentle face no step reaches joins
them. Each support so put down keeps the next a coverage of that spacing away in a set of its
own, so it keeps away only its own kind: an overhang above it is still held at its spacing.
They come back as `Reason::Steady`, built — with those under a part held at too few
places — to the automatic profile's `steady_shape`, shipped as `auto-stable`: a 0.18 mm
contact on the shapes' 1.2 mm pillar, and a 4.5 mm pad.

The window sizes a step by the same thresholds and lets the two upper ones be tuned.

## Consequences

A wide shelf takes a heavy support even with nothing on it, the face rising off a shelf is
held for a few millimetres where it would have hung, and parts with broad gentle undersides
get a few thin supports there, in a group the notice counts apart, which merge into the
trunks beside them like any other of their width. Both sizes are read per layer: a shelf
grown over several layers in thin slivers is many small steps, and a narrow slope tall
enough to matter is skipped; reopen if such a shelf comes out under-held or such a part is
seen to rock. Two more reaches per layer and two sets of buckets per sample keep the
`tables` benchmark about 6 % over a run that does not size.

## Alternatives considered

### Size a step by the model above it, as an overhang

No new measure, but a shelf with nothing on it would take the lightest head however wide
its peel.

### Hold gentle faces everywhere, at the regular spacing

Simpler, but every model would carry supports, as many as an overhang's, on faces the
profile's own angle says stand by themselves.

### Let a steadying support keep everything away at its spacing

One set of supports, but a steadying support low on a part would then keep the overhangs
for 7 mm above it unheld.

### The option that won, and what it costs

A second and third angle beside the profile's, a fourth group on a sized plate, placement
that differs between a sized run and one built to a group, and a slope's size that is only
an approximation of its area.

# 0326. Size a step by the resin it puts out, and hold the faces just above it

- **Status:** Accepted
- **Date:** 2026-10-22

## Context

A step — material put out further than the resin bridges in one layer — is held as an
overhang (ADR 0080), and an automatic profile sizes an overhang by the model above it
(ADR 0325). That measure misses what a step is: a shelf cured onto film in one exposure,
whose pull is its own area, however little stands on it. And the face that rises from the
edge of a shelf leans out over a hold that is only one layer old: at a lean the overhang
angle lets pass on a cured wall, it sags off the shelf before the shelf has set.

## Decision

A run that sizes its supports holds steps apart from overhangs. `unsupported` then splits
the shelf put out over the layer below from the lean measured over the rise, and also
answers the faces that lean past a gentler angle, `step_lean_deg` (20 degrees from
vertical), but not past the profile's, leaving out the foot of a shelf that its wall
carries.

A step's size is the resin its piece puts out in that one layer, area times layer height.
The automatic profile gives the least that counts as a step at all, and where the second
and third shape begin — 0.1, 7 and 12 mm³ — and how far above a step of each size its
reach goes, 1.5, 2.4 and 3.2 mm. A smaller step is the overhang it was. Each sample of a
step is kept, while the climb is within its reach, as a ball of that radius above it
(`step.rs`, bucketed by the tallest reach, dropped once passed). An overhang or a gentle
face inside such a ball is held as a step of the largest one reaching it, and comes back
as `Reason::Step` with its resin; a gentle face no step reaches is left alone. The window
sizes a step by the same thresholds and lets the two upper ones be tuned.

A run built to one group, with no automatic profile, finds and holds steps as before.

## Consequences

A wide shelf takes a heavy support even with nothing on it, and the face rising off a
shelf is held for a few millimetres where it would have hung. More supports go on
terraced models when sized. Sizing costs one more reach over the fresh material of each
layer and a look into nine buckets per sample: 4 % on the `tables` benchmark. The size is
read per layer, so a shelf grown over several layers in thin slivers is many small steps
rather than one large one; reopen this if a shelf printed that way comes out under-held.

## Alternatives considered

### Size a step by the model above it, as an overhang

No new measure, but a shelf with nothing on it would take the lightest head however wide
its peel.

### Hold gentle faces everywhere

Simpler, and not tied to steps, but every model would carry supports on faces the
profile's own angle says stand by themselves.

### The option that won, and what it costs

Two more fields' worth of numbers in the automatic profile, a second angle beside the
profile's, and placement that differs between a sized run and one built to a group.

# 0342. Search further for ground before a support is run onto the part or left out

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

A front with nothing under it leans for room (ADR 0078): three reaches out to the merge
distance, eight ways round. Past that it is run onto the model under it or left unstood,
and users saw a run report dozens of supports with no room to stand under a wide part
that had open plate a little further off. Established practice searches for ground with a
strut of about 25 mm at 45 degrees before giving up.

## Decision

`stand` tries, in order: down and the lean of ADR 0078 onto a face up to 45 degrees, the
same onto one up to 75 degrees, and then `search_for_ground` onto each in turn. It goes on
from the merge distance out to where a strut `GROUND_SEARCH_MM` = 25 mm long at the
profile's lean ends — 17.7 mm across at 45 degrees — every 1.5 mm across and 16 ways round,
and takes the nearest knee whose strut is clear and under which a trunk lands. Merges keep
the short lean: the search runs only for a front that is about to be given up.

## Consequences

A support under a wide part now leans to the plate beside it instead of being run onto
the part or left out. The knee is longer, so more of such a support hangs at 45 degrees,
and bracing reaches it as it reaches any trunk. A blocked front costs up to 192 more tries,
each a beam and a landing; it is paid only by fronts nothing nearer could stand. On top of
ADR 0341, `cargo bench -p core-supports --bench branch` grows the lifted 40 mm ball in
8.1 ms against 6.5 ms, and the one on the plate in 95 ms against 82 ms.

## Alternatives considered

### Raise the merge distance

It would also widen every merge, changing the trees of every model to help a few points.

### A path of several knees

Would find ground round a corner, but each bend is a joint to check and a lever on the
peel; one long knee covers the case users hit.

### The option that won, and what it costs

A second constant beside the merge distance that a profile cannot tune, and a lean that
can now carry a support further from what it holds than any merge would.

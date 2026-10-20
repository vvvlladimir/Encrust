# 0317. A run tells how far it has got through its `Cancel`

- **Status:** Accepted
- **Date:** 2026-10-20

## Context

Hollowing one model takes from one second to twenty, and the window showed a bar that only
counted models: for one model it ran without a share, with no word of what was happening or
how far it had got. Every heavy function of `core-volume` already takes a `Cancel` and
asks it between the pieces of its work — rings and tiles of the field, layers of tiles of
the surface — which are exactly the places a count can be taken. Cores may not know the
window (`.claude/rules/architecture.md`, rule 1).

## Decision

`Cancel` also carries an optional `&Progress`: `Cancel::when(..).reporting(&progress)`.
`Progress` is three atomics — the `Step` in hand, pieces done, pieces in the step — written
by the run from any of its threads and read by the window each frame, so nothing is sent per
tile. The steps are Preparing, Seeding, Carrying (tiles of the rings and of the settling
passes), Refining (stored tiles), Surfacing (layers of tiles) and Shaping; an exact boolean,
which Combine tries before a field, adds Reading (models), Combining and Finishing, the
engine itself counting nothing inside. The window turns them into one share with fixed spans
per step, weighed on a figurine and a bust at two walls and two precisions, and names the
step. An exact boolean reads its models in about the time the engine then takes, and
building its mesh back is a tenth of the run. It states no time left: with spans that are
averages, a figure in seconds was wrong by half on a thin wall.

## Consequences

Relief over a field, Combine and Split report the same steps for free; the window's `Worker`
carries a `Progress` for every tool, and a run that counts nothing leaves it unbegun, so its
panel keeps a running bar. A press of several models runs them side by side and is not
counted, since one count for all would jump between them. A count costs one relaxed
`fetch_add` per tile; a field has thousands. The spans are averages, so on a thin wall,
where carrying weighs more, the bar moves unevenly. Shaping is one uncounted step: an open
floor or a lattice waits at 98 % until its booleans are done. Reopen if a step needs its own
count — the floor's booleans — or the spans drift from what profiles show.

## Alternatives considered

### A `Progress` argument beside `Cancel`

Cleaner in name, but every heavy function would take a second argument threaded through
the same calls, and the two always travel together.

### Messages over a channel per step or per tile

What slicing does with layers. Per tile it would be thousands of messages a second; per
step it would say nothing within the step that takes most of the time.

### The option that won, and what it costs

`Cancel` now means "the run's handle", not only cancellation, so the name undersells it.
Steps are a fixed list in `core-volume`; a run that is not a field reports none of them.

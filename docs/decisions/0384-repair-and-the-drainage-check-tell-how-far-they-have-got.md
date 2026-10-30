# 0384. Repair and the drainage check tell how far they have got

- **Status:** Accepted
- **Date:** 2026-10-30

## Context

Repair already handed its run a `Progress` (ADR 0317), but `core_volume::repair` began no
step, so its bar ran without a share even on a wrap of seconds. The drainage check had no
`Progress`, no cancel and its own channel. Measured in release on four overlapping balls of
0.16 to 1.4 million faces, a wrap spends about 16 % mending, 17 % finding crossings, 56 %
in the exact union and 11 % building and checking the result; on real models that never
reach the union, mending and crossings split it evenly. A check cuts every layer of every
model at the print's height, and the layer count is known before the first is cut.

## Decision

`Step` gains Mending and Crossing. A repair on the mesh alone reports Mending; a wrap
Mending, Crossing, then Combining and Finishing around the exact union; a deep repair's
second mend is Finishing. The window spans them Mending 0–0.2 and Crossing 0.2–0.45,
leaving Combining and Finishing where the exact boolean has them. Repairs started together
are one share — those finished plus the shares of those going, over all started — named
by the step of the first going, with "Repaired k of n" over it.

The drainage check runs on the window's `Worker`: it counts layers as windows are scanned,
and models done; its share is models done plus the layers of the one in hand, over all
models. It stops between windows when asked, and the Drain tool has a Cancel.

## Consequences

Repair names what it is doing and moves at three points; within a step it does not move,
so on a wrap that reaches the union the bar waits at 45 % for half the run. A copy on the
plate counts at once, as it is never cut. Reopen if mend or the exact engine learn to
count, or if the spans drift from what a profile shows.

## Alternatives considered

### A Progress of the window's own for repair

It would keep `Step` to fields and booleans, but the steps are where `core-volume` already
stops to ask its `Cancel`, and a second carrier would duplicate ADR 0317's.

### Counting the drainage check in `Step`

`Step` is `core-volume`'s and the check is the slicer's; its counter is two atomics in the
job, not a step of a field.

### The option that won, and what it costs

`Step` is now three runs' list, and a hollow, which never begins Mending or Crossing, must
still name them. The spans of a repair are one measurement on synthetic balls.

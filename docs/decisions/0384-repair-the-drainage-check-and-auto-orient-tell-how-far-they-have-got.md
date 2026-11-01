# 0384. Repair, the drainage check and Auto-orient tell how far they have got

- **Status:** Accepted
- **Date:** 2026-10-30

## Context

Repair already handed its run a `Progress` (ADR 0317), but `core_volume::repair` began no
step, so its bar ran without a share even on a wrap of seconds. The drainage check and
Auto-orient had no `Progress`, no cancel and a channel of their own. Measured in release on
four overlapping balls of 0.16 to 1.4 million faces, a wrap spends about 16 % mending, 17 %
finding crossings, 56 % in the exact union and 11 % building and checking the result; on
real models that never reach the union, mending and crossings split it evenly. A check cuts
every layer of every model at the print's height, and the layer count is known before the
first is cut. An orientation search scores its candidates and cuts a shortlist in 3 to 19 %
of a run and prints the finalists, in parallel, for the rest; `core-plate` cannot name
`core-volume`'s `Progress` (architecture rule 5).

## Decision

`Step` gains Mending and Crossing. A repair on the mesh alone reports Mending; a wrap
Mending, Crossing, then Combining and Finishing around the exact union; a deep repair's
second mend is Finishing. The window spans them Mending 0–0.2 and Crossing 0.2–0.45,
leaving Combining and Finishing where the exact boolean has them. Repairs started together
are one share, named by the step of the first going, shown over the bar.

The drainage check runs on the window's `Worker`: it counts layers as windows are scanned
and models done; its share is models done plus the layers of the one in hand, over all
models. It stops between windows when asked, and the Drain tool has a Cancel.

An orientation search takes a `core_plate::Watch`, as a field takes a `Cancel`: a function
told the `Phase` (Looking, Proving), the pieces done and the pieces it has, from any thread,
that answers false to stop the search with `PlateError::Cancelled`. Looking counts the cuts
of the shortlist; Proving counts three steps a pose — supports stood, stack measured, islands
counted. The window averages its models and spans Looking 0–0.12, Proving the rest; the
Auto-orient section has a Cancel. The CLI passes `Watch::never()`.

## Consequences

Repair names what it is doing and moves at three points; within a step it does not move,
so on a wrap that reaches the union the bar waits at 45 % for half the run. A copy on the
plate counts at once, as it is never cut. In a proof the first step of the poses, which
stand their supports side by side, takes a third to a half of it: the bar waits at 12 %
for 2 to 4 s on a million faces, then climbs evenly. `finalists`, `prove` and
`orient_for_print` take one more argument. Reopen if mend, the exact engine or the
supports learn to count, or if the spans drift from what a profile shows.

## Alternatives considered

### A Progress of the window's own for repair

It would keep `Step` to fields and booleans, but the steps are where `core-volume` already
stops to ask its `Cancel`, and a second carrier would duplicate ADR 0317's.

### Moving `Progress` down for the search

A shared crate under `core-volume` and `core-plate` would let the search report as a field
does, but it moves a type every tool uses across a crate boundary for one caller that needs
two phases and a count.

### The option that won, and what it costs

`Step` is now two runs' list beside the search's own `Phase`: three ways of telling
progress, each owned where the work is. The spans of a repair are one measurement on
synthetic balls, and those of a search three models'.

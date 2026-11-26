# 0485. A searched pose is kept as its turn

- **Status:** Proposed
- **Date:** 2026-11-26

## Context

A revision keeps a client's `set_pose` as the search and the choice by number (`pick`, ADR 0458).
Every later revision ran the search again. The lift, the supports and the machine change its
ranking, so a new lift gave another pose under the same number, and the model turned with no one
asking.

## Decision

Once a revision whose pose is a search is checked, the server keeps the turn the check chose with
it (`stood_deg`, not in the tool's schema). Later revisions stand the model by that turn,
`--rotate`, and run no search. A new `set_pose` asks anew, and `compare_poses` searches with no
turn kept. Two poses that differ by the turn kept alone are the same ask, so no edit turns the
window's model again. A pose over several models, as after a cut naming some objects, keeps none.

## Consequences

The model keeps the pose the client chose and looked at. A turn kept is not proved again on a
new printer or resin; the client asks `set_pose` again for that. Reopen if a later step should
re-rank the poses on purpose.

## Alternatives considered

### Turn `set_pose` into `Rotate` at once

Rejected: the revision would forget it was a search, which the window stands as an orient
proved on the job's printer (ADR 0458), and its keep-clear points.

### Search again and pick the nearest turn

Rejected: it runs a search no one asked for and can still jump.

### The option that won, and what it costs

A field the client never sees, and a pose not checked again when the machine changes.

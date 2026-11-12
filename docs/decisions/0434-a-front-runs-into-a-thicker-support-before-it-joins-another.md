# 0434. Run a front into a thicker support before it joins another front

- **Status:** Proposed
- **Date:** 2026-11-11

## Context

Branching pops merge candidates from one heap, highest meeting point first; a front may run
into a carrier — a strut or trunk of a thicker group already stood (ADR 0248) — or meet
another front. Two thin tips 1 mm apart meet half a millimetre down, higher than either meets
a thick trunk 3 mm away, so they joined first and the joint leaned on to the trunk: a strut
out to the joint and a second one back, a zigzag beside a trunk either tip could reach.

On `crustling` stood 5 mm up, seven fronts did this with the carrier in reach and room on it.

## Decision

**A carrier candidate pops before every front-to-front candidate**, then each kind highest
meeting point first. A front that can run into a thicker support within its branch angle,
half its merge distance and its longest strut does so from where it is; the room a carrier's
trunk has for tips and the strut's clearance are checked as before, and a front refused
there merges as it did.

## Consequences

- Thin tips beside a thick trunk end on it one strut each, never by way of a joint.
- More struts end on a trunk, up to the tips its group lets it carry; fewer thin joints.
- The bench corpus holds as before; `hollow-skeleton-duck` and `spiderman-edit` take
  0.03–0.05 ml more.
- Reopen if a trunk run into from many sides breaks at its joints on the peel.

## Alternatives considered

### Refuse a merge only where a carrier is nearer than the joint

Keeps more joints, and needs a second look-up for every merge the heap judges.

### Carriers first, and what it costs

Two thin tips that could share one strut to the trunk take two.

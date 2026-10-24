# 0344. Let a tip's head lean up to 60 degrees off vertical

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

ADR 0249 held everything that carries a support to 45 degrees off vertical, the head of a
tip included: a face whose normal is further from straight down than that was left by a
head at 45 degrees, off its normal, and the plane tip's disc was laid onto it as an
ellipse (ADR 0330). Users saw such tips lie on the surface rather than meet it square on.
Established practice lets the tip lean up to 60 degrees; the 45 degrees that keeps a strut
from sagging is about long bodies, and a head is a few millimetres hanging from the face
it holds.

## Decision

A head follows the normal of its face while that is within 60 degrees of straight down,
`MAX_HEAD_LEAN_PER_DROP` = tan 60°, and leaves a steeper face, a wall or a slope looking up
the way it looks at 60 degrees. A head turned for room tries 15, 30, 45 and 60 degrees off
its direction, each no flatter than 60. Trunks, knees and the strut into a thicker support
keep 45 degrees (ADR 0249), and so does the long lean for ground (ADR 0342).

## Consequences

Faces between 45 and 60 degrees off straight down are met square on, with a round disc;
steeper ones at 60 rather than 45 degrees, so their disc is less stretched. A head now
reaches further sideways from its contact — 1.7 mm across for a 2 mm head against 1.4 mm —
and ends nearer its face, which the checks of ADR 0328 and 0341 still hold out of the
model. A head at 60 degrees hangs more of itself as an overhang; reopen if heads are seen
to sag or snap on the peel.

## Alternatives considered

### A profile setting for the head's angle

What some slicers offer, but every shipped preset would take the same value; a constant
until a user needs another.

### Keep 45 degrees and bite the tip in instead

A bite reaches the face whatever the angle, but the presets touch without biting by
choice (ADR 0323), and a stretched disc still holds along one edge.

### The option that won, and what it costs

Two lean limits in placement instead of one, and heads that stand out further from the
surface, so a support under a crowded face meets its neighbours' heads a little sooner.

# 0345. End a plane tip in a half ball of its contact, centred on the contact

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

A plane tip ended on a flat disc pushed its bite into the face (ADR 0243) and laid onto
the face's plane (ADR 0330). At the shipped bite of nought (ADR 0323) that disc ended
exactly on the surface: tip and part shared no material, on a curved or coarse surface
the disc stood on one facet and off the rest, and a head leaning off the normal laid an
ellipse up to twice the contact's width (ADR 0344). Established practice draws the tip
cone as a capsule: its narrow end is a half ball of the contact's radius centred on the
contact point, moved in by the bite, so at a bite of nought half the ball is in the part,
and any face through its centre cuts it in a circle of the contact's width.

## Decision

A plane tip ends in a half ball of `contact_diameter_mm / 2`, drawn in four rings, centred
`contact_depth_mm` into the face along the head's own axis; the cone runs from its equator
out to the pillar. The ball's pole is held to half the wall behind the contact (ADR 0125):
where the ball would reach further, its centre comes back out of the part. The tip a
trunk standing on the part ends in is rounded the same way. Nothing is laid onto a face
any more but the plain ends of a small pillar (ADR 0331). The profile key stays `plane`.

This supersedes the flat disc of ADR 0243 and the laying of a plane tip's disc onto its
face in ADR 0330.

## Consequences

Every plane tip shares a contact radius of material with the part at a bite of nought —
0.12 mm for the automatic light shape, 0.3 mm for Medium — and leaves a round mark exactly
as wide as its contact however its head leans. The scar is that much deeper; the bite
field now moves the ball's centre, not the end of the tip. The figurine bench of ADR 0341
lost its one torn neck. Reopen if the scar shows on finished prints; a flatter cap would
then be the knob.

## Alternatives considered

### A pin along the normal, sunk a fixed least depth

Tried and withdrawn: a second solid per tip and a minimum written in the code that the
profile cannot reach.

### A bite in the shipped presets

Leaves the disc flat, so a leaning head still leaves an ellipse, and on a coarse surface
still stands on one facet.

### The option that won, and what it costs

The name `plane` no longer describes the shape, and a tip on a wall thinner than its
contact's diameter is pulled back out until it only half touches.

# 0339. Let only a tip meet the model: a half ball seated on its face, its head square on or behind a pin

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

A support may touch the part only at its tip; anything thicker that meets the model cures
into it and leaves a scar. A head — the `[top]` segment widening from the contact to the
pillar — was asked about along its axis alone (ADR 0249), held to 45 degrees off vertical,
and turned for room or kept where it was when nothing cleared, so under a curled finger or
beside a lip the cone ran into the model or lay on the face past the tip: its mark grew from
the contact's width to several times it. A plane tip ended in a flat disc square to its head
(ADR 0243): at the shipped bite of nought (ADR 0323) it shared no material with the part,
and on a curved or coarse surface stood on one facet and off the rest. A contact picked near
the edge of a face kept its place with half its tip past the edge, curing onto the wall
beyond. Users asked that a support touch by its tip alone, its mark the contact's width, or
not be placed.

## Decision

**A plane tip ends in a half ball** of `contact_diameter_mm / 2`, drawn in four rings,
centred `contact_depth_mm` into the face along the head's own axis; the cone runs from its
equator out to the pillar. Any face through the centre cuts it in a circle of the contact's
width. The ball's pole is held to half the wall behind the contact (ADR 0125): where it
would reach further, its centre comes back out. The tip a trunk standing on the part ends
in is rounded the same way. The profile key stays `plane`.

**A head leaves its face square on** while the face's normal is within 45 degrees of
straight down (`STEEPEST_SQUARE_HEAD_DEG`). Past that it is turned back towards straight
down to 45 degrees, by at most 33 (`MAX_HEAD_LEAN_DEG`): a wall's head leaves at 57 degrees
off vertical. Behind the half ball a turned head runs at the contact's width for `r · tan θ`
along its axis before it widens (`pin_mm`), so a face cut slantwise meets only the ball and
the pin: the mark reaches `r / cos θ` on one side, its width growing by `(1 + 1/cos θ) / 2`,
9 per cent at 33 degrees. Everything under the neck bends and leans by the rules it had.

**A tip is seated.** It is seated when 8 points round its circle, each looked for along the
face's normal within half the radius, find a face within 60 degrees of the contact's own: a
facet of a coarse curve counts, a wall meeting it at an edge does not. A contact not seated,
or whose head has no room, moves to the nearest spot of a face looking down within 0.05 mm
of its height, on rings 0.05 mm apart every 30 degrees, out to the larger of 0.3 mm and its
radius and one ring, where a head has room and the contact is seated. Where no spot seats
it, a contact with no room moves to the nearest spot with room within 0.3 mm — the reach a
contact moves onto an island by (ADR 0329) — and one with room stays.

**A head is swept whole, or the support is not placed.** `neck_of` sweeps the cone with
`clearance_mm` round it from the end of its pin; the stretch of trunk just under a head,
which the trunk's sweep skips, is swept bare. A support whose head meets the model where it
ends up, or rises so nearly straight up that a trunk from its neck stands less than a
pillar's and a contact's radius off the contact, has no head: `columns` leaves it out and
the panel counts it as having no room to stand, unless its profile has small pillars, which
then hold it (ADR 0340).

This supersedes the flat disc of ADR 0243 and the head's 45 degrees of ADR 0249.

## Consequences

Every head's mark is the contact's circle, and off a face past 45 degrees up to 9 per cent
wider on one side; at a bite of nought tip and part share a contact radius of material —
0.12 mm for Auto's light shape, 0.3 mm for Medium — so the scar is that much deeper, and the
bite field moves the ball's centre. A contact near an edge leaves a whole round mark on one
face; on a rib narrower than the tip nothing seats, so it keeps its place. Contacts in a
crevice, beside a lip or on a face looking up that once stood on a slanted head now step
aside, stand on a small pillar or are reported. `neck_of` sweeps one head per contact; every
column costs 8 more rays, and a contact not seated up to 12 times its rings in head sweeps.
Reopen if marks show on finished prints, steep faces still grow hooks, or prints show tips
moved off what they were placed to hold.

## Alternatives considered

### Lean the head up to 60 degrees, or turn it for room

Tried in turn: each lays the widening cone on the face slantwise, the scar this removes.

### Every head along its normal, whatever way it looks

Tried: off a wall the head ran out sideways a whole `[top]` and the trunk hooked down under
it. Turning every head down by up to 33 degrees instead bent gentle overhangs' heads that
users had found straight.

### Lean the head with no pin, or shorten it off steep faces

The cone's own mark is a third wider at 30 degrees; a short head stands the trunk as close
to the wall as the head is short, and more contacts would have no room.

### A pin sunk a fixed depth, a bite in the presets, or a tip shrunk at an edge

A second solid per tip and a minimum no profile reaches; a disc left flat and standing on
one facet; a mark and a hold other than the profile asked for.

### The option that won, and what it costs

Faces a turned head used to hold are now left to a small pillar or unheld, by the user's
choice that a support touching by more than its tip is worse than none; 45 and 33 degrees
are constants no profile sets; `plane` no longer describes the shape, and a tip on a wall
thinner than its contact's diameter is pulled back out until it only half touches.

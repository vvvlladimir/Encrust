# 0354. Move a contact in until its whole tip lies on its face

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

ADR 0348 moves a contact only when its head has no room, and only 0.3 mm. A contact picked
near the edge of a face, with room for its head, keeps its place even when half its tip hangs
past the edge: that half cures onto the wall beyond and leaves a scar on a second face, or
holds nothing. The tip is 0.4 to 1.0 mm across on the shipped profiles, so 0.3 mm is often
too little to bring all of it back onto the face.

## Decision

- **Seated.** A contact is seated when 8 points round its circle, each looked for along the
  face's normal within half the radius, find a face within 60 degrees of the contact's own:
  a facet of a coarse curve counts, a wall meeting it at an edge does not.
- **Moved in.** A contact not seated moves to the nearest spot of a face looking down within
  0.05 mm of its height where a head has room and the contact is seated, out to the larger
  of 0.3 mm and its radius and one ring (rings 0.05 mm apart, every 30 degrees).
- **Room alone.** Where no spot seats it, a contact with no room for a head moves as in
  ADR 0348, within 0.3 mm, and one with room stays where it is.

This widens the step aside of ADR 0348 for a contact that ends seated.

## Consequences

A contact near an edge leaves a whole round mark on one face. On a rib narrower than the
tip nothing seats, so it keeps its place. A move past 0.3 mm can leave a small island it
was put on only where another face lies within one radius at the same height; an island
wider than the tip seats it on itself first, as the rings go nearest first. Every column
costs 8 more rays, and a contact not seated up to 12 times its rings in head sweeps.
Reopen if prints show tips moved off what they were placed to hold.

## Alternatives considered

### Shrink the tip at an edge

Keeps the place but changes the mark's size and the hold the profile asked for.

### Leave it to placement

Placement picks from the layers alone and does not know the tip's size on the face.

### The option that won, and what it costs

More raycasts and head sweeps per contact, and a contact may sit up to a radius and
0.05 mm from where it was placed.

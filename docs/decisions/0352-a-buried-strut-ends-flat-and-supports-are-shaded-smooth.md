# 0352. End a strut buried in its joint flat, and shade supports smooth on twelve sides

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

A strut ended in a six-ring dome centred on its joint (ADR 0238), and the tube leaving the
joint started under a dome of its own, no narrower. Where that tube runs on, the strut's dome
lies wholly inside it and is never seen, yet it was a third of the strut's triangles.
Supports went from 12 sides to 24 to look round, because the viewport shades every mesh
flat (`docs/design/viewport.md`). A figurine's 1 768 supports came to 2.75 million
triangles, more than twice the model's, sent to the card on every growth.

## Decision

- **A buried end is flat.** A strut ends in a flat disc on its joint where the tube leaving
  the joint is no narrower and runs on at least twice the strut's width. A ball of the
  wider radius about the joint then holds the disc, and the outside is the one the dome
  gave. Otherwise the dome stays.
- **Supports are shaded smooth.** A `ModelInstance` can be `smoothed`. Its mesh then goes to
  the card with every corner carrying the normals of the faces round it, each weighted by
  its angle there. Faces turning more than 50 degrees from each other are kept apart, so a
  pad's rim stays sharp, and the fragment blends that normal across the face. Models stay
  flat.
- **The shipped profiles go back to 12 sides.**

This supersedes ADR 0238's dome at the end of every strut.

## Consequences

The figurine's supports fell from 2.75 to 1.03 million triangles. The flat ends alone took off
a quarter, on the same 24 sides. A tube of 12 sides reads round, though its outline still
shows its corners up close. A strut over a trunk shorter than twice its width keeps its
dome. Slicing sees the same solid outside. A profile a user saved keeps the sides it was
saved with. Reopen if the outline of a thick trunk reads faceted on screen.

## Alternatives considered

### Ray-traced impostors

Exact round silhouettes from two triangles a segment, but a pipeline of its own for cones,
domes, flares and pads, and the section and drain passes taught them all again.

### Keep 24 sides and shade flat

Twice the triangles for an outline barely rounder, with every facet still lit apart.

### The option that won, and what it costs

A second normal carried through the model shader, an upload that averages normals and is
slower than the flat one, and a strut end that now depends on the tube under it.

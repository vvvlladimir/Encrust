# 0341. Draw supports round and filleted on twelve sides, end a buried strut flat, and shade them smooth

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

Supports looked like pencils: twelve sides lit flat (`docs/design/viewport.md`), joints
closed by domes of three rings that show outside the strut at a bend as a coarse lip, and a
pad met by a straight 45-degree cone (ADR 0130) — or by nothing, since the presets set the
flare as wide as the pillar. A notch where a pillar meets its pad is where the peel starts a
crack. Every strut ended in a dome centred on its joint (ADR 0238) even where the tube
leaving the joint, no narrower, hides it whole: a third of a strut's triangles. Going to 24
sides to look round brought a figurine's 1 768 supports to 2.75 million triangles, more than
twice the model's, sent to the card on every growth.

## Decision

- **Round joints.** A dome is drawn with six rings.
- **A filleted flare.** The flare is a quarter circle tangent to the pillar and to the pad, in
  four steps, still rising as far as it widens and never wider than the 45-degree cone it
  replaces; every shipped preset has one (ADR 0323).
- **A buried end is flat.** A strut ends in a flat disc on its joint where the tube leaving
  the joint is no narrower and runs on at least twice the strut's width; a ball of the wider
  radius about the joint then holds the disc, and the outside is the one the dome gave.
  Otherwise the dome stays.
- **Supports are shaded smooth.** A `ModelInstance` can be `smoothed`: its mesh goes to the
  card with every corner carrying the normals of the faces round it, each weighted by its
  angle there. An edge whose faces turn more than 50 degrees is a crease, and the faces round a
  corner that smooth edges join share one normal, so a pad's rim stays sharp and a dome's pole
  is one point; the fragment blends that normal across the face. Models stay flat.
- **Twelve sides** stay the default `facets` and what every shipped profile has.

This supersedes the straight cone of ADR 0130 and ADR 0238's dome at the end of every strut.

## Consequences

Supports read as round on the screen and the pillar's foot carries no step. The figurine's
supports, drawn at 24 sides, fell from 2.75 to 1.03 million triangles at twelve with flat
ends; the flat ends alone took off a quarter. A tube of 12 sides reads round, though its
outline still shows its corners up close; reopen if a thick trunk's outline reads faceted.
A strut over a trunk shorter than twice its width keeps its dome, and slicing sees the same
solid outside. Volume tests compute a flare step by step instead of as one frustum.

## Alternatives considered

### A fillet height of its own, as a profile field

Another number to tune for a shape the flare already describes; the flare's width sets it.

### Twenty-four sides, shaded flat

Twice the triangles for an outline barely rounder, with every facet still lit apart.

### Ray-traced impostors

Exact round silhouettes from two triangles a segment, but a pipeline of its own for cones,
domes, flares and pads, and the section and drain passes taught them all again.

### The option that won, and what it costs

A second normal carried through the model shader, an upload that averages normals and is
slower than the flat one, a strut end that depends on the tube under it, and a flare that
widens every pad's foot.

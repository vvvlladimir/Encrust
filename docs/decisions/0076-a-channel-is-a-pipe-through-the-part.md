# 0076. A channel is a pipe through the part, not a slot in it

- **Status:** Superseded by 0188 (waiting for a second run only)
- **Date:** 2026-09-23

## Context

A channel was only ever a cut: `drill` meshes its tube and the fill rule subtracts it
(ADR 0075). The hollow run knew nothing about it, so the cavity was cut wherever it liked
and the tube crossed it. What came out was not a duct but a gap: the channel merged into
the cavity along its whole length, ate the infill it passed through and opened a groove in
the shell where it ran under the surface.

A user drawing a channel is drawing plumbing. The part has to keep its own wall around the
tube, the way it keeps one around a spar that a `Blocker` covers.

## Decision

A channel blocks the cavity. `sleeves` turns each leg of a channel's spine into a
`Blocker` of the tube's radius plus the wall thickness, and the callers that start a
hollow run pass them in beside the blockers a user placed: `ObjectHollow::asking` in the
window, `HollowArgs::settings` in the CLI. The cut itself is unchanged — the channel is
still subtracted by `drill` — so what prints is the tube with a wall of its own.

`Blocker` is therefore a capsule rather than a ball: `from`, `to` and a radius, with
`Blocker::ball` for the single point a click drops and `depth_at` for how far inside one a
point lies. One shape covers both, and `blocked` keeps its one line of arithmetic.

Because the sleeves ride in `HollowSettings`, `is_stale` already reads them: digging a
channel on a hollow model marks its shell stale and the window asks for another run. Until
that run the channel is a bare tunnel between its points, which is what a cut on a model
with no cavity always was.

## Consequences

A channel drawn through a hollow model comes out as a pipe: solid around it, open along
it, and the infill it passes is cut back to the sleeve rather than sliced open. The wall
of the pipe is the wall of the model, so it follows the thickness the user typed.

A sleeved channel is a duct between its own two mouths and no longer drains the cavity it
passes through. A channel meant to empty a pocket has to end in that pocket, or the pocket
needs a drain hole of its own; the drainage check is what says which.

A channel costs a hollow run now, where it used to cost nothing. The signal to reopen is a
user who wants the old behaviour — a channel that deliberately opens into the cavity all
along — which would need a flag on `Channel` rather than a second kind of blocker.

## Alternatives considered

### Carry the channels in `HollowSettings` beside the blockers

A second list, a second shape in `blocked`, and `HollowSettings` would carry a cut again,
which ADR 0075 had just taken out of it. The sleeve is a blocker in everything but name.

### Mesh the pipe's wall and append it, leaving the cavity alone

Cheap — no second hollow run — but the wall would stand in the cavity as a separate body
overlapping the shell, and the infill would still be cut through it. Wrong at the field,
patched at the mesh.

### The option that won, and what it costs

Blocking is done at the field, so a channel is only a pipe after a hollow run that knew
about it. Place one on a finished shell and the model is stale until it is hollowed again,
which on a real model is seconds of work for a tube that was already drawn on screen.

# 0322. Take a hole out of its own model exactly

- **Status:** Accepted
- **Date:** 2026-10-21

## Context

A hole or a channel was never cut out of the mesh. Its body was appended wound inward and
`CUT_WEIGHT` deep (ADR 0188), so the winding rule left no material in it wherever the plate
was filled, and the viewport discarded every fragment inside any cut on the plate (ADR
0073). Both acted on the whole plate: a model set down in another's hole lost that hole's
worth of itself on screen and in the printed file, though nobody drilled it.

## Decision

Holes and channels of every outline are taken out of their own model through `core-csg`:
`core_volume::drilled` reads the model — its shell once hollowed, lattice and all — mended
first as a plug's model is (ADR 0321), and subtracts the bodies `drill` laid, read once over
and unioned. `bake` takes them out of each model this way and out of the support trees they
reach (`drilled_pieces`), and appends no cut. The window does the same off the drawing
thread for every model whose cuts or mesh changed, one run a model at a time and none during
a drag, keeps the result in `ModelHollow` against the meshes it came from, draws it with the
cavity's faces carried over, and hands it to `bake` so a slice does not cut again. An undo
step keeps no drilled mesh: the plate put back is drilled again unless the one it replaces
was drilled for the same cuts. A model the engine refuses even mended keeps the winding cut
and the per-fragment discard, and the window says so.

## Consequences

A hole reaches nothing but what it was drilled in, on screen, in the file and in a picked
split. Taking three holes out of a half-million-face ball costs 0.26 s, paid once per edit
on a worker; until it lands, and on a model the engine refuses, the old cut stands in, with
its reach over the whole plate. A slice taken before it lands cuts the model itself. The
drainage check still counts the winding cut, which is the same answer for one model.

## Alternatives considered

### A cut of each model's own, kept on the winding rule

Scoping the shader's cuts per model mends the screen alone; the plate is one fill, so the
file needs each model filled apart and then merged, a pass a model through the slicer's
hottest path.

### The option that won, and what it costs

A boolean of the whole model on every edit of its holes, a drilled mesh as large as the
model held beside it, the winding cut kept for a model the engine refuses, and a second way
of cutting a hole to maintain until no model falls back to it.

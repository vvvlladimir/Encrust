# 0188. A cut outweighs what it lands in, and a channel stays a pipe

- **Status:** Accepted
- **Date:** 2026-10-05

## Context

A hole or channel is a closed body appended wound inward, and the fill rule takes it out
because it counts `-1` (ADR 0071). That is only enough where one body stands. The lattice is
bonded half a wall deep into the shell, its walls overlap at every junction, and a support's
tip stands inside the part: there the winding is 2 or 3, a hole leaves 1 or 2, and the layer
keeps strips and specks of resin across the hole. The section cap counts the same way (ADR
0074), so it was painted over the hole as well. The window showed an open hole and the
printer got a plug.

A channel dug on a model that is already hollow made its shell stale (ADR 0076): until a
second Hollow the tube crossed the cavity, its drawn wall stopped a wall's depth past each
mouth (ADR 0073), and its bends had no wall at all. The model read as open where a pipe had
been asked for.

## Decision

`drill` lays every cut body `CUT_WEIGHT` = 8 times over itself, each copy on vertices of its
own. Eight outweighs the most that stands at one point in practice — the shell, the lattice
bonded into it, three walls meeting at a corner, a support's tip.

A channel dug or cleared on a hollow model rebuilds its shell on the spot, at the numbers it
was built at (`ModelHollow::rebuild`, `HollowTool::rebuild`). Since its cavity therefore
always keeps a sleeve round the pipe, `bores` draws a channel's wall its whole length and a
ball's wall round every bend that stands in the model; a hole's wall still stops at the wall
a cavity left.

## Consequences

What a hole or channel covers is gone from the layer, the drainage check and the section cap
alike, whatever else stands there, and the fill rule, the stencil and the scan need no change.

The contour-area volume, the CLI's `sliced volume`, now takes a cut off eight times where it
used to take it off once — about 2 % on a 30 mm ball with a 4 mm hole. The cured volume the
file states is counted from pixels and is unaffected. Signed volumes of a cut body read eight
times its size.

Digging a channel on a hollow model costs a hollow run without a second press. Blockers stay
manual: each click would start a run.

Reopen if a plate stacks more than eight bodies at one point, or if the stencil's 127 is
reached by eight-deep cuts along one ray — sixteen holes in line.

## Alternatives considered

### Carry the cuts apart to the rasteriser

The slicer cuts the solid and the cuts separately, and a pixel is material where the solids
wind positive and no cut covers it. Exact for any overlap, but it reaches through the bake,
the slicer, the rasteriser, the drainage check and the CLI, for a bound nobody meets.

### Take the overlaps out instead

Stop the lattice short of the shell and weld its junctions. Touching surfaces leave slivers
in the mask, and supports and overlapping models still stack.

### The option that won, and what it costs

A fixed bound with a contour-area figure that drifts. Eight copies of a few hundred triangles
cost nothing to slice; the drifting figure is a report line, and the file's own is exact.

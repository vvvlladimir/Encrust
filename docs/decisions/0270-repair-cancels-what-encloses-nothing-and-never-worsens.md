# 0270. Repair cancels what encloses nothing, patches by least area, and never worsens

- **Status:** Accepted
- **Date:** 2026-10-14

## Context

The asked-for repair (ADR 0194, 0195, 0205) came back unsound on three of five broken
models in a test set. Every duplicate face in them was one of a pair over the same three
vertices, wound opposite ways: a wall of no thickness that encloses nothing. Dropping one
face of a pair left the other walking its edges the same way as a neighbour, so
`remove_unbalanced_faces` cut out more faces around it. `fill_holes` then laid the same
triangle back, and the pair was there again. A lone triangle came back the same way,
patched by its own reverse.

Projected ear clipping also fell back to a fan on 14 loops that do not lie flat: a hole
across a curve whose projection folds over itself.

The window asked about every broken model as it landed, before the user had seen it.

## Decision

**A pair of faces wound opposite ways over the same corners goes whole. Of a stack wound
one way, the first face stays.** `duplicate_faces` decides it, and both `diagnose` and
`remove_duplicate_faces` read it, so what is counted and what is removed agree. This
changes nothing the surface encloses and unbalances no edge, so the import runs it in the
window and on the command line, after welding and before orientation.
`remove_duplicate_faces` returns the indices it dropped, and a texture map drops the same
faces. The window counts them among `faces_removed`, so the project format is unchanged.

**An edge used an odd number of times is a boundary.** The patch walks it the way the
surface is short of. A hole's edge is used once and a fin's foot three times, so a fin is
closed by its own reverse, and the two cancel.

**A loop of up to 256 corners is patched by the triangulation of least total area,** by
dynamic programming over its spans, with slivers forbidden. A longer loop is ear clipped in
its plane as before, and fanned only if that fails.

**`remove_unbalanced_faces` is gone.** The asked-for repair drops doubled faces, orients,
fills, drops doubled faces again and orients. If the result is less sound than the input,
counting boundary edges, degenerate faces and duplicates, or nothing is left of it, the
mesh stays as it came and the user is told.

**The window no longer stops to ask on import.** A broken model carries its mark, and a
card at the foot of the stage offers Repair and Keep as it is until one is chosen. Repair
also sits in the mark's popup and the row's menu, and a model it cannot mend counts as kept.

## Consequences

All five models in the set come back sound. Puck_Base is sound on import and never needs
the button. Fans fall from 14 to 1. On a 200k-triangle sphere, dropping duplicates takes
2.0 ms, against 5.3 ms with a full sort, and `diagnose` takes 8.6 ms, against 14.7 ms
before. The run is counted out by lowest corner, as `edge_uses` is.

A zero-thickness sheet with nothing on either side, once filled and cancelled, vanishes
rather than printing as a wall of no thickness, which it never could have. A model that is
nothing but such a sheet stays as it came.

## Alternatives considered

### Keep the tangle removal for fins

It closed a fin by cutting it out and patching the cut. The odd-edge walk closes it with
its reverse, cuts nothing, and works the same way for holes.

### Patch every loop by least area

Its cost is cubic in the corners. The 1 024-corner flat loop in the set would take about a
second, while ear clipping takes milliseconds.

### The option that won, and what it costs

A pair's two faces both go, so a file that meant one of them as surface loses it. That
face was enclosing nothing with its twin either way. Least area is not fairness: a hole
across a sphere comes out as the tightest skin, not as the sphere.

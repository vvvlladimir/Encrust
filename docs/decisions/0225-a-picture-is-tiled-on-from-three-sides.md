# 0225. Tile a picture onto a model from three sides

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

ADR 0116 presses a texture into the field through the map a model's file came with. Most
printable models carry none: an STL has no UVs, and the Relief tool was greyed out for
them. The Relief panel offers a picture chosen from a file, tiled at a size and turned by
an angle (ADR 0218). Without a map, something has to say where on the picture each point of
the surface reads.

## Decision

`core_volume::press_tiled(mesh, bvh, image, Tiling { tile_mm, angle_deg }, settings)`
reads each field point's height through a triplanar projection on the model's own axes,
blended by the nearest face's normal to the fourth power; the rest of `press` — the field,
the band, the coarsening retry — is shared. The window reads the picture with the `image`
crate it already has, as grey, read down to 1024 pixels a side, and keeps it on the Relief
tool alone: the project does not write it down, as it writes no relief settings (ADR 0191).

## Consequences

Any model takes a relief, and the existing texture path is unchanged. The tiles run along
the model's axes, not the plate's: turning a model turns its pattern with it, which is what
a texture should do. Where two projections blend the picture is seen twice, softened; on a
pattern with strong lines that shows as a seam at 45°. A project reopened has the relief
pressed but no picture to press again with.

## Alternatives considered

### Pressing onto one picked face only

Exact on a flat face, and nothing anywhere else; most surfaces worth texturing are curved.

### Unwrapping the model for a map

A real UV unwrap is a library and an algorithm family of its own, for a pattern that does
not need to be continuous.

### Triplanar, and what it costs

A face at 45° to two planes reads both pictures at half weight. A pattern meant to line up
across an edge will not.

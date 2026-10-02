# 0071. Fill where the winding number is positive

- **Status:** Accepted
- **Date:** 2026-09-22

## Context

A drain hole crosses the boundary of the model: part of the body that cuts it is inside
the wall, and part of it stands in the air outside. Everything this workspace subtracts is
a closed body appended wound the other way — the cavity (ADR 0059), a blocker's ball, now
a hole — and under the non-zero rule of ADR 0008 the part standing outside counts `-1`,
which is not zero, which prints. A hole would come with a plug of resin sitting over it.

That is why every subtraction so far had to stay inside the solid: `open_bottom` clips its
prism against the model's own field, and the lattice intersects its spans so a strut can
never reach out through the wall. A hole cannot obey that rule and still be a hole.

The alternative was already named and rejected in ADR 0008 — fill where the winding is
positive — on the grounds that the coordinate flips in `PixelSpace` negate the winding and
the correction would have to be threaded through every mapping. It does not: the flips are
two axis reflections and one panel mirror each way, so the orientation is a parity of
three booleans, known before a single edge is built.

## Decision

A pixel is material where the winding number is **positive**. A body wound the other way
subtracts wherever it lands, inside the model or outside it.

`PixelSpace::reverses_winding` is that parity, and the rasteriser reverses each ring as it
maps it, so the sweep always runs in a space where counter-clockwise is material — the
convention `Contour::winding` already states. `Edge::direction` counts `+1` upwards to
match.

## Consequences

Drain holes and channels are plain meshes appended to the model: exact at any layer height,
twelve triangles for a tube, no field, no clipping, and nothing to go wrong at the mouth.
The cavity, the blockers and the lattice keep working unchanged, because all three are
already inside the solid where the two rules agree.

Overlapping solids still expose their union, which is what ADR 0008 was protecting.

What changes is what happens to a mesh wound inward: it used to print as a solid, and now
it prints as nothing. Both binaries call `orient_outward` on import, so this is only
reachable through `slice --no-validate` on an inverted STL — where a silent inversion was
already the bug, and is now visible on the first layer instead of after the print. Two
test fixtures were built inside-out and had gone unnoticed for exactly that reason.

Reopen this if a contour source appears whose orientation is not knowable — the rule now
depends on it in one direction rather than in neither.

## Alternatives considered

### Clip every hole against a field of the model

Build the model's field over the hole's bounding box, intersect, extract, append. Keeps the
non-zero rule. Rejected because the extracted cap lands within half a voxel of the true
surface, so the hole is left under a membrane of material half a voxel thick — a trap for
resin over the hole meant to let it out.

### Trim the hole against the mesh with rays

Cast a ray per generatrix, cap the tube on the hits. Exact where the rays land, and
polygonal between them, which leaves slivers of material across part of the mouth on any
curved surface.

### The option that won, and what it costs

The fill rule now depends on the sign of a winding, not just on its being non-zero, and
that sign is reconstructed from a parity rather than being immune to the mapping. A future
transform that reflects a coordinate — a second panel mirror, a flipped plate — has to be
counted in `reverses_winding` or the whole mask comes out empty. That is a loud failure
rather than a quiet one, but it is a failure the old rule could not have.

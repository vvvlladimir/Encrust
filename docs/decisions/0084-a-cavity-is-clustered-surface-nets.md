# 0084. Extract a cavity as surface nets clustered onto the lattice

- **Status:** Accepted
- **Date:** 2026-09-24

## Context

Marching cubes gave the cavity of a 150 mm ball at 0.2 mm **5 023 476 triangles**, about
three for every cell its surface crossed. That number follows the lattice and not the
shape: a sphere and a crumpled scan of the same area cost the same. It is appended to the
model, sliced, and drawn, so every consumer pays for it.

Surface nets was tried first on its own — one vertex a cell rather than one a cut edge —
and gave **5 023 480**. No reduction at all: a smooth surface crosses about half again as
many lattice edges as cells, and a quad per crossed edge is two triangles, which cancels
the saving exactly. That measurement is what said the reduction has to come from merging,
not from the contouring scheme.

## Decision

Surface nets for the contouring, and **vertex clustering on the lattice** for the
reduction.

A cell the surface passes through gets one vertex, the mean of the crossings of its own
edges, and the gradient of its trilinear field as a normal. A crossed lattice edge gets one
quad, joining the four cells around it, claimed by the cell that owns the edge.

A tile is then walked as its own halves, quarters and eighths. A block becomes **one**
vertex when every cell in it stands within half a voxel of the block's shared plane and
faces the same way as it; otherwise the block is split and each part asked again. So a flat
stretch of cavity comes out as a few large facets and a tight one keeps its cells. A block
never crosses into another tile — a tile's lowest cell is a multiple of eight — so no tile
has to agree with any other about where a facet ends.

Half a voxel is not a new tolerance: it is what the field itself promises (ADR 0082) and
where marching cubes already placed the surface.

Two corrections the measurements forced:

**The facet is pushed back out.** The mean of a curved patch's cells lies inside the
surface, because a chord is; clustering a whole cavity that way shrinks it, and a mould —
whose cavity *is* its printed outside — came out 3% light against its closed-form volume.
The cells' normals fan by the patch's width over its radius, so the radius is what that fan
says, and the vertex is lifted by `radius · (1 - |mean normal|)`, clamped to the slack. On
a flat stretch the normals do not fan and nothing moves.

**Folded quads cancel.** Clustering can map two different quads onto one triangle. Keeping
both leaves every edge of it used four times, which `diagnose` calls non-manifold and which
slices into garbage. Two copies wound the same way are one piece of surface written twice
and one is dropped; two wound against each other enclose nothing and both are. The table
that does it is cut back a layer at a time, beside the one ADR 0064 already keeps.

There is no 256-pattern table any more, generated or otherwise, because surface nets has no
cases to tabulate.

The merge of ADR 0064 is otherwise unchanged: vertices are still named by a lattice
coordinate — now the cell rather than the cut edge — and tiles are still merged in
ascending Z, X, Y, which is also the order a quad reaches back in.

## Consequences

The cavity of the 150 mm ball goes from 5 023 476 triangles to **314 076**, and the whole
hollowed model from 9 562 104 to 1 364 520. Extraction itself costs the same 0.3 s. What is
downstream — the slice stack, the viewport buffers, the drain scan — all pay the smaller
number.

The cavity's triangle count now follows its shape. That is the point, and it is also the
thing to watch: a scanned model whose cavity is detail everywhere will cluster very little
and come back to roughly what marching cubes cost. The ball is the friendly case and 16x is
not a number to expect everywhere.

`the_sweep_answers_what_the_hierarchy_would` bounds the field; the extraction is bounded by
`a_smooth_cavity_costs_far_fewer_triangles_than_the_cells_it_crosses`, which asserts both
the reduction and that the surface is still closed. Closed is the one that matters: a
cavity is sliced, and an open contour is a ruined print. Moving the slack, the span cap or
the cancelling rule without rerunning that test is how this breaks.

## Alternatives considered

### Marching cubes, then quadric decimation

Garland–Heckbert after today's extraction, with a target ratio. The best mesh quality of
the three and an unbounded knob. Rejected because it pays for all five million triangles
before removing them, and holds them beside their replacement — which is the peak memory
ADR 0064 was written to bring down.

### Adaptive dual contouring on an octree

Merge cells down an octree wherever the field is planar. The largest reduction available.
Rejected on the cracks: neighbouring blocks at different levels leave T-junctions that need
explicit stitching, and a crack in a cavity is an open contour. Clustering has no such
seam, because it never changes the topology — only which vertex a cell stands on.

### Merge quads into bigger quads instead of clustering vertices

The shape OpenVDB's `volumeToMesh` adaptivity takes. Rejected for this codebase because a
merged quad's edge spans several unmerged neighbours' vertices, so the border has to be
fanned against them; that is the T-junction problem again, in the one place it cannot be
tolerated.

### The decision above, and what it costs

Clustering is blunt. It moves vertices to the mean of a patch rather than solving for where
they should be, so it loses a sharp feature outright — a chamfer inside a cavity comes out
rounded to the nearest facet. A cavity has no sharp features worth keeping, which is why
this is affordable here and would not be for the model's own surface.

It also folds quads, which is why the cancelling rule exists at all, and that rule is a
repair rather than a prevention: nothing stops the fold, it is undone afterwards. A model
that folds far more than a ball does would carry a table proportional to how much it folds.
Facets are bounded to half a tile only because a tile is walked as its own halves, which is
a structural bound and not a considered one.

# 0092. Paint a patch as a set of faces, fill it on the plate's grid

- **Status:** Accepted
- **Date:** 2026-09-25

## Context

Supports have been placed one click at a time or over the whole model. What every other
resin slicer offers in between is a patch: a surface painted with a sphere brush or taken
by angle, with a grid of supports projected onto it, or an area selected with a handful of
tools and auto-support run inside it alone. A patch has to be stored
somewhere, survive the model being moved, turned and scaled, and be filled at a spacing
the print sees rather than a spacing the mesh does.

Nothing in this workspace can offset a polygon, which is what a rim inside a painted
outline would otherwise need (ADR 0088).

## Decision

A patch is a `Region`: one bit per face of the model, in the model's own space. A click
already resolves to a face, the set rides every placement of the model for nothing, and a
fill reads the same triangles the slicer will. It is painted two ways: a brush of a
radius in plate millimetres, measured face by face against the model's own space, and a
flood from one face across shared edges while the normal stays within an angle of the
face that was clicked.

`project` fills a region in two halves. The inside is sampled on a grid of the plate at
the asked-for spacing, taking the lowest surface over each cell, so the spacing is what
the print sees however finely the patch is meshed. The rim is walked along the edges the
region ends on — the faces across them are outside it.

Every candidate then passes one crowding rule: nothing is put within three quarters of the
finest spacing asked for, measured against the supports already standing as well as
against the rest of the fill. The rim goes down first, and the supports on the model seed
the test, so a rim of triangles shorter than the spacing does not become one support each
and a second press of Fill adds nothing on top of the first.

## Consequences

Painting costs one `Bvh` query a stroke and a flood costs one `Adjacency`, which
`core-geometry` now builds and nothing keeps: a fill is one press of a button, and holding
a face adjacency beside every model would cost the memory of a third mesh for it. A model
with a few hundred thousand faces pays that build on the click, not on the drag.

The resolution of a patch is the mesh's own. A 10 mm square modelled as two triangles is
painted whole or not at all, and a brush cannot take half of one. That is the signal to
reopen: a user who cannot paint a coarse model finely enough wants the patch stored as
something finer than a face.

A rim is the boundary of a set of faces, not an offset of an outline, so it runs along the
model's edges and jitters by a triangle where the mesh is coarse. It costs nothing and it
needs no polygon boolean.

Because the crowding rule takes the finest of the two spacings, a rim asked for at 1 mm
also loosens nothing inside: the grid is thinned to the rim's distance where the two meet.

## Alternatives considered

### A point cloud, or a texture over the surface

What a painting tool would do if the mesh were not the thing being filled. Rejected: both
need a parameterisation the mesh does not carry — this workspace has no UVs until step 16
— and a point cloud has to be re-resolved against the surface on every edit.

### Filling over the triangles instead of over a grid of the plate

Sampling each triangle at its own spacing needs no projection and keeps the fill on the
surface exactly. Rejected because the spacing then follows the mesh: a finely meshed nose
gets ten times the supports of the coarse cheek beside it.

### The decision above, and what it costs

A patch is as coarse as the model's triangles, and the grid fill is on the plate's axes,
so a patch on a steep wall is sampled at its horizontal spacing and comes out denser down
the slope than across it. A vertical wall painted whole gets one row of contacts, which is
the right answer for the wrong reason.

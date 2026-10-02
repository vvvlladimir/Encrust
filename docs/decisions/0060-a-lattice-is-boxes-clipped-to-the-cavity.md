# 0060. A lattice is boxes clipped to the cavity, not a field

- **Status:** Accepted
- **Date:** 2026-09-21

## Context

`core-volume` already turns a mesh into a field and a field back into a mesh, so the
obvious way to fill a cavity is to keep going: write the lattice as a periodic function of
position, take the `max` of it and the cavity, and march the result. One operator, no new
machinery, and any pattern that can be written as a function comes for free — including
the gyroid, which is the pattern a field is *good* at, because it is a curved surface with
no flat part anywhere.

That is what this step shipped first, and it does not survive contact with the parameter
ranges a slicer has to offer. A gyroid's sheet area per unit volume is about `3.09 / cell`
and marching cubes writes about `2 / voxel²` faces over it, so

```
faces ~ C * volume / (cell * voxel^2)
```

with `C` measured at 21 for a gyroid and 8 for a grid of walls. Nothing in that is
bounded. On a 60 mm ball with a 2 mm wall the gyroid cost 619 MB at a 10 mm cell and a
1 mm wall, 3.46 GB at 5 mm and 0.6 mm, and 6.90 GB at 4 mm and 0.4 mm on a machine with
8 GB; a 1 mm cell with a 0.2 mm wall is 224 million faces and the process dies rather than
the call. Worse, the sheet is genuinely curved, so slicing it produced thousands of
grazing contours per plate — 4 032 closed over a gap on that ball against 21 for a grid.

The slicers that do not have this problem avoid it for a reason worth stating plainly:
**they have no gyroid.** Their structures are scaffolds, hives and grids, and they stand
perpendicular to the platform. A vertical hexagonal tube is a
flat-sided prism. It does not need to be found on a lattice, because it can be written
down: twelve triangles per piece, whatever the resolution. The cost follows the number of
cells, which is thousands, rather than the resolution, which is unbounded.

## Decision

The lattice is meshed directly and never voxelised. There is no gyroid.

Three patterns, the three a resin slicer offers:

- **Hive**, hexagonal tubes standing on the plate;
- **Grid**, square tubes standing on the plate;
- **Scaffold**, struts along all three axes on a cubic lattice — the only one that braces
  sideways.

All three are the same shape underneath: a **closed box**. A wall is a box; a strut is a
box. Boxes overlap rather than being welded, because the non-zero winding rule already
unions them — the same trick a support's struts have been meshed with since ADR 0041.

The cavity is still a field, and it is still what the lattice is clipped against, but by
**sampling rather than meshing**. A vertical wall runs along a segment of the pattern's
footprint; that segment is walked in chunks, and each chunk asks the cavity's field for
the stretches of its own vertical line that lie inside. The spans of a chunk are
intersected into the run before it, so a box is only ever *shorter* than the cavity under
it and can never reach out through the wall, where the winding count would have nothing
to cancel it. A run is broken only where holding it would cost more than a lattice step of
height, so a wall crossing the middle of a cavity is one box and only its ends are chased
in detail.

The user states **density**, not a wall thickness. The thickness is the inverse of each
pattern's own solid fraction, so a 20% grid of 5 mm cells is a 0.53 mm wall and the number
that means something — how much resin this saves — is the one being set.

The cavity's own lattice is set by **precision**, from 0 to 1, the way every resin slicer
states it. It moves the spacing geometrically between 0.8 mm and 0.05 mm, and a wall is
always given four voxels whatever the setting, because a wall the lattice cannot resolve
is not a wall.

## Consequences

The lattice costs what its cells cost. On the 60 mm ball at a 2 mm wall, peak memory and
wall clock for the whole run, against the gyroid it replaces:

| | Before | After |
|---|---|---|
| default lattice | 619 MB, 4.2 s | 329 MB, 3.0 s |
| 2 mm cells, 40% | about 30 GB | 1.84 GB, 13.6 s |
| 1 mm cells, 50%, precision 1 | about 150 GB | 5.14 GB, 74.9 s |

Nothing in the window's ranges dies any more. The corner of them is slow and large, and it
is a honeycomb of 1 mm cells in a 60 mm ball, which is 37 139 contours on a layer — real
geometry rather than waste.

Slicing got quiet. The same plate that produced 4 032 contours closed over a gap produces
three, because every face of the lattice is now axis-aligned or upright rather than a
curved sheet grazed by a slice plane.

Merging the runs is what made it affordable, and it is worth writing down separately: a
box per chunk would put one contour per chunk on every layer the wall crosses. On a grid
of 2 mm cells that was 8 928 contours a layer; merged, it is 83.

We lose the gyroid, and it was the better structure — isotropic, self-supporting, draining
in every direction, and stiffer than a honeycomb for its weight. A hive only carries load
up and down. That is the price of the decision and it is a real one.

The signal to reopen this: a pattern that cannot be written as boxes — anything curved or
graded — would want the field back, and would have to come with a way of bounding its own
face count before it could be offered.

## Alternatives considered

### Keep the field and bound it with a triangle budget

What this step tried before ripping it out: predict the face count from the cavity volume,
coarsen the lattice to fit a budget, refuse what still does not fit. It works, in that the
process survives, and it was measured at 6.90 GB to 1.92 GB on the worst case that still
built. It is also a number tuned to one machine sitting in a library crate that knows
nothing about the machine, it coarsens silently so the user gets struts they did not ask
for, and it still refuses settings that are perfectly reasonable on a small model. It
treats the symptom.

### Slice the lattice per layer and never mesh it at all

What nTop and the implicit-slicing literature do: evaluate the pattern on each layer's
plane and contour it there. Memory is one layer. It would also make the gyroid affordable
again. It is a second slice engine, a second path through the preview and the viewport,
and a model that is no longer one mesh — most of what ADR 0059 and the pipeline behind it
assume. Worth doing one day; not worth doing to rescue one pattern.

### The option that won, and what it costs

The clip is approximate where the field's was exact. A wall's end is the intersection of
the spans over a chunk, so it stops a fraction of a chunk short of the shell and is then
pushed back out by half the wall thickness to meet it. Neither number is the cavity's
actual surface. It lands inside the wall, where it is harmless, but "inside the wall" is
the guarantee rather than "on the cavity".

A run is broken on a rule of thumb — more than a lattice step of height lost — and that
rule decides how many boxes a curved cavity gets. It is not derived from anything.

Density is exact for a grid and a hive and approximate for a scaffold, where the struts
overlap at every node and the `3 t² / c²` it inverts ignores that. At 30% the scaffold is
a few percent denser than it says.

And the honest one: the boxes overlap, so the lattice mesh's own volume over-reads and
cannot be used to report what the infill costs. The number reported is the density times
the cavity, which is true by construction and therefore proves nothing.

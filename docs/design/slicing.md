# Slicing a mesh into contours

How `core-slicer` turns a triangle mesh into a stack of closed 2D contours, and why the
degenerate cases that break a naive slicer do not arise here.

## The pipeline

1. `SliceSettings::plane_heights` lays out one sampling plane per layer, or
   `Windows::planes_of` lays out `samples` of them inside each layer's band.
2. `ZBins` buckets the faces by the Z range they span, so a plane only tests the faces
   that can reach it. A face the range does not reach is left out of the index entirely.
3. `plane::crossing` intersects one face with one plane and returns a directed segment.
4. `stitch::stitch` chains those segments into closed contours.

Layers are independent, so step 3 and 4 run in parallel across planes with `rayon`.

## Several planes to a layer

One plane a layer is the middle of its band, and a feature thinner than a layer is then
either fully printed or fully lost depending on which side of that plane it falls: the tip
of a cone, a fin, a thin rib. `SliceSettings::samples` splits the band into that many equal
parts and takes the middle of each.

The planes are ordered by how close they sit to the middle of the band, and the leading one
becomes the layer's `contours`, the rest its `extra`. So everything that reads a
cross-section — supports, the drainage scan, Preview — sees what it saw before, while only
the rasteriser looks at `extra`. With an odd count the leading plane is exactly the band's
middle; with an even one it is off centre by `band / 2n`, which is 12.5 µm at two planes
and 0.05 mm layers.

The union is taken on the runs, not on the contours: taking it on the contours needs a
polygon boolean, which the workspace does not have (ADR 0088, ADR 0089). See
`docs/design/rasterisation.md` for how the planes are combined and ADR 0114 for why they
are combined by brightness rather than by adding their windings.

Cost follows the plane count in work but not in wall time: `slice_at` receives every plane
of the window in one call, so `rayon` spreads them the same way it spreads layers. Slicing
a 14 400-triangle sphere into 400 layers on eight cores took 0.43 s at one plane a layer and
0.45 s at five.

## A window at a time

`slice` cuts every plane at once and is what a caller wants when it is going to keep the
stack — the window's Preview tab, which the user scrubs. A caller that is going to write
the stack out and drop it asks for `layer_heights` and then hands chunks of them to
`slice_at`, which cuts exactly those planes and indexes exactly their Z range.

That is what the CLI does, 64 layers at a time and settable with `--slice-window`. It
matters twice on a dense model: one window's contours rather than the stack's, and an
index over one window's geometry rather than the model's — an infill strut is listed in
every bucket it spans, which on that 60 mm honeycomb made a 541 MB index out of 4.17
million faces. See ADR 0066.

A shorter window holds less and rebuilds the index more often, each rebuild walking every
face. The result does not change: the same model gives the same contours whatever the
window.

## Where the sampling plane sits

A `LayerPlan` is the boundaries between layers, lowest first, plus the height the mesh
stops at: layer `n` occupies `[bounds[n], bounds[n+1])` and the plane that represents it
sits in the middle of that band, clipped at the top to the mesh. A uniform stack is the
plan whose boundaries are `z_min + n·h`; an adaptive one is the same shape with its own
numbers (ADR 0091).

Sampling the middle rather than an edge matters because model vertices land on layer
boundaries all the time: CAD work is done on a 0.05 mm grid and printed at 0.05 mm layers.
A plane on the boundary would meet those vertices head on. A plane in the middle of the
band meets them almost never, and when it does the classification rule below handles it
exactly anyway.

Clipping the *plane* is what keeps a partial top layer. Without it the last plane of a
7.305 mm model at 0.02 mm layers would sit at 7.31 mm, above the material, and the tip
would come out empty. The *boundary* is not clipped: the plate travels a whole layer
whether or not there is material all the way up, and that is what the file records.

## Adaptive thickness

The stair step a layer leaves on a surface is `h·|n·ẑ|`, the layer height times the cosine
of the angle between the surface normal and the build direction: zero on a vertical wall,
the whole layer height on a horizontal face. Inverting it against a cusp target gives the
thickness a height may take, and `adaptive_plan` takes the largest whole number of the
thinnest layer that every slot a layer spans allows.

Faces are scattered into a 1D grid of that thinnest layer, each slot keeping the steepest
lean crossing it, so the cost follows the triangles rather than the height of the model.

A flat face is the degenerate case: the formula asks for the thinnest layer going, when
what the face wants is a boundary on itself, where the error is nil. Flat faces are
therefore left out of the grid and their heights recorded, and a layer that can land on one
does. A surface a degree off horizontal is not flat by this test and pays the thin layers —
that is the honest cost of reading the decision off a normal.

## The classification rule

Every vertex gets one bit against the plane at height `z`:

```
above = vertex.z > z
```

Strictly greater, so **a vertex sitting exactly on the plane counts as below it**. An edge
is crossed when the bits at its two ends differ.

That one rule removes every degenerate case:

- **A vertex exactly on the plane.** The bit is well defined, so no zero-length segment
  and no duplicated point appears. Where a face passes through such a vertex, the segment
  ends exactly on it.
- **A face touching the plane with one vertex only.** All three bits come out equal, so
  the face yields nothing rather than a degenerate point.
- **A face lying flat in the plane.** All three bits are false: nothing. Horizontal faces
  never contribute, and they do not need to — the walls around them do.
- **An edge lying in the plane at the foot of a wall.** The wall's faces still cross, and
  the interpolation lands exactly on that edge, so slicing at `z_min` returns the model's
  footprint rather than nothing.

The bit depends only on the vertex and the plane, never on which face is asking, so two
faces sharing an edge can never disagree about whether it is crossed.

It also makes the count of crossings structural: around a triangle the classification can
flip zero or two times, never one. A crossing face always yields exactly one entry edge
and one exit edge, so the "what if only one edge crosses" branch does not exist.

The convention is half-open in Z: `[z_min, z_max)`. Slicing at exactly `z_max` returns
nothing, which is why no sampling plane is ever placed there.

## Segment direction

The segment runs from the edge the face walks *down* through the plane to the edge it
walks *up* through it. On a mesh wound counter-clockwise seen from outside — which
`core-geometry::orient_outward` guarantees — that puts material on the left of every
segment, so outer contours come out counter-clockwise and holes clockwise.

Taking the direction from the face is what makes nested holes work without any
point-in-polygon test. The signed area of the finished contour only labels it.

## Stitching by topology

A contour point is identified by the mesh edge it sits on: both vertex indices, lower
first. On a closed, consistently wound mesh every crossed edge is walked down by one face
and up by the other, so it appears exactly once as an entry and once as an exit. The map
from entry edge to segment is a permutation, and following it walks each loop exactly
once, in `O(n)` with no distance tolerance anywhere.

This is why welding at import is not optional. On an unwelded STL every face has its own
copies of its vertices, no edge is ever shared, and nothing stitches.

Points are interpolated from the lower vertex index to the higher one regardless of which
face asks, because `v0 + t·(v1 - v0)` is not symmetric in floating point. Both faces on a
shared edge therefore produce bit-identical coordinates, and adjacent segments meet with
no gap at all.

Chains are walked from their heads first — the entry edges more segments leave than
arrive at. Starting anywhere else would cut a single broken contour into two.

Where two sheets of surface share an edge, as they do where a cavity touches itself, the
edge is entered twice and left twice. Every segment leaving it is kept, and a chain arriving
there takes the one that turns furthest left, which traces two loops touching at a point as
two loops rather than one figure of eight (ADR 0186).

## What a broken mesh does

Nothing here fails a slice. What the mesh made the slicer paper over is counted in
`Sliced`:

| Counter | Meaning |
|---|---|
| `open_contours` | A chain ran out of faces and was closed with a straight jump. |
| `degenerate_contours` | A chain enclosed no area and was dropped. |
| `unlinked_segments` | An edge carried a second entry: the surface branches there. The chain still follows it. |

`Sliced::is_clean` is all three at zero. The CLI prints them and `--strict` turns them into
a non-zero exit code.

## Cost

Bucketing faces by Z is what keeps the cost independent of model height — about eight
times faster than testing every face per plane. `cargo bench -p core-slicer`.

Writing a sliced file cuts the stack once: the pass that writes also counts it, and the
resin volume the header states is laid down at the end (ADR 0067). On a 60 mm ball packed
with a 1 mm honeycomb that is 220 s of CPU against 205 s for cutting the stack whole, for
a peak of 0.7 GB against 1.90 GB.

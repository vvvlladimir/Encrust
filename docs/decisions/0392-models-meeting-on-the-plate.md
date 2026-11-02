# 0392. Warn of models that meet on the plate, by their faces and by winding, and of supports past it

- **Status:** Proposed
- **Date:** 2026-11-01

## Context

Two models standing into each other, or a support of one reaching into another, slice
into one piece: the slicer fills by positive winding, so what overlaps is united. Nothing
breaks and nothing says so; the user finds two figures fused on the plate. Supports are
grown per model and bite into their own model by design, and the rafts of neighbours
often overlap without harm. A warning that fires on those is soon ignored. The fit was
measured on the bare model, so supports or a raft past the build volume were clipped unsaid.

## Decision

**`core_plate::crossings` names each pair of models that meet, and where**: a face of
one, or of its supports and raft, crosses a face of the other, or one stands wholly inside
it. Each `Meeting` carries the seam their faces cut and the places they meet: the box
round each run of the seam, runs a millimetre apart being one place, round faces lying on
each other, or the inner body's box.
Touching counts, faces lying on each other included; a gap of any width does not. The
bodies of one model never meet each other, and supports never meet supports.

It runs in three steps, so the cost follows the faces where bodies meet, not the plate:

- the bodies' boxes go into `Buckets` 10 mm across, and only pairs sharing a bucket whose
  boxes meet go on;
- `core_geometry::placed_crossings` asks one body's `Bvh` for its faces in the box both
  stand in, carries each into the other's space and asks the other's `Bvh`, testing pairs
  in f64 on `rayon`, and answers every pair of faces that cross. Unlike `self_intersections` an edge through a triangle's rim counts:
  between two meshes a crossing at a round offset lands there;
- with no face crossing, a body whose box lies in the other's is inside where one of its
  vertices is, by the other's `Winding`.

A `Bvh` or `Winding` a caller does not hand in is built the first time a pair needs it.

**The window traces the seam in the scene's `meets` red, in every tool**: each segment two
crossing faces cut each other along, between the points where the edges of each pierce
the other, an even spread of at most 10 000 a pair, as a duplicate in place cuts some
hundred thousand, drawn as the Repair tool's marked edges are, a line a few pixels wide pulled a
hair towards the eye, cut where the section cuts the models and left out of the render
view, which shows the print and not its faults. The Check tool lists each pair that meets under *On the plate*, with
the models past the build volume or in the inset, before any layer is read, and the
models' rows carry *Meets*; a press on either flies the camera to a place, the next press
to the next. The CLI names the pairs and fails `--strict`.

**Past the plate measures what prints**: `core_plate::PlateFit` takes a model with its
supports and raft where it was put down, for the window's striping and Check rows and the
CLI's *On the plate* answer alike; `--strict` fails past the build volume, the inset still
only warns (ADR 0371).

## Consequences

The window and the CLI ask one function and agree. A cavity is not inside: a model
standing in another's hollow does not meet it. A model meeting another on purpose, to be
fused, is warned of too; the warning never stops the slice. A gap thinner than resin can
hold apart is missed; a tolerance would reopen this.

## Alternatives considered

### The footprint grid, or the layers' masks

`Footprint` already rasterises each model and its supports at 1 mm. Two shadows sharing a
cell is not two bodies meeting: a figure leaning over its neighbour shares cells and
prints apart. Masks per model and per layer would be exact to the pixel, but the stack is
cut for the whole plate at once, and cutting it per model for a warning doubles a slice.

### Closest points under a clearance

Warning of a gap under, say, 0.3 mm would catch the models resin bridges, but each face
asks a distance rather than a crossing, and the clearance would need a home and a reason.

### Faces and winding, and what they cost

A first check of a large plate builds a `Bvh` for every support mesh near another model,
and a `Winding` for a body another may stand in; a move redoes the pairs it touches only
where the caller keeps the hierarchies. Faces crossing exactly at a shared vertex or edge
are touching and missed, which the vertex-inside step does not cover for two bodies only
grazing.

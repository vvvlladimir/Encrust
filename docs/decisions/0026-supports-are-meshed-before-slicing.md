# 0026. Mesh supports and merge them into the model before slicing

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

A support has to end up in the printed file. There are two places it can enter the
pipeline. It can be geometry, meshed into triangles and merged with the model before the
slicer sees either, or it can be a 2D primitive drawn into each layer after the model has
been sliced and before it is rasterised.

The pipeline already has properties that bear on the choice. `PlaneSliceEngine` fills by
non-zero winding (ADR 0008), so two closed solids that overlap slice as their union
without a boolean step. `core-raster` emits runs rather than pixels (ADR 0020) and
anti-aliases by exact pixel area (ADR 0021), so a circle drawn as a separate primitive
would need its own coverage maths to match the edge quality of a sliced contour. The
stack is rasterised in windows and streamed out (ADR 0010, ADR 0012), so anything added
per layer has to be added inside that window loop.

The viewport also has to show the supports, and it draws meshes.

## Decision

Supports are geometry. `core-supports` turns a list of contact points into one closed
`Mesh` of columns in plate coordinates, and `encrust-app` appends that mesh to the model
in `merge_visible` before the slicer is called. Nothing downstream of the merge knows a
support exists.

`core-supports` therefore depends on `core-geometry` and `printer-profiles` and on
nothing else. It does not know about layers, pixels or files.

## Consequences

Slicing, anti-aliasing, the layer preview, the `.goo` writer and the PNG stack all handle
supports with no code of their own: a column is a solid like any other. The overlap where
a tip bites into the model is resolved by the existing fill rule rather than by a boolean
operation. The viewport draws the column mesh through the pipeline it already has.

The cost is triangles. Every column is a lathe of `facets` sides around five rings, so a
plate with a thousand supports carries a few hundred thousand extra faces through the
slicer. The slicer bins faces by Z (ADR 0007), and a column spans most of the stack's
height, so those faces land in many bins at once.

Reopen this if a benchmark shows the merged mesh dominating slicing time on a realistically
supported plate. The way out would be to slice the model and the supports separately and
merge the contours, not to go back to drawing primitives per layer.

## Alternatives considered

### Draw supports as 2D primitives into each layer

Cheap: a column is a circle at a known centre, and its cross-section per layer is a
formula rather than a mesh intersection. It was rejected because every other stage would
have to learn about it. The preview rasterises contours, the `.goo` writer takes runs, and
the anti-aliasing is area-exact against polygon edges; a circle primitive would need a
parallel implementation in each. Branching supports in step 7c are not extrusions of a
circle at all, and there is no primitive that describes them.

### Mesh the supports and merge them, which is what we do

The honest cost is the triangle count above, and that the support mesh is rebuilt in full
whenever the model moves, because where a column lands depends on where the model is.
Rebuilding is one downward raycast per point over the whole mesh, which is linear in faces
today; step 7b brings the point counts that justify an acceleration structure and the
benchmark to size it with.

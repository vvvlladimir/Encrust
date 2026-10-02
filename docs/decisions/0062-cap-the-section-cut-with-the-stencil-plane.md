# 0062. Cap the section cut with the stencil plane

- **Status:** Accepted
- **Date:** 2026-09-22

## Context

ADR 0061 cuts the viewport's models by discarding, in the fragment shader, every fragment
above the rail's height, and left the solid open at the plane. It named the cost in its own
consequences: a cut model reads as an open shell, and capping it would take a stencil pass
or the model's own contour fanned into a face.

That open cut is what a user reads as wrong. A cut hollow shows its far inner wall, curved
and lit, with the lattice of an infill standing behind it; the eye reads a bowl rather than
a part sliced through. Every MSLA slicer this project is measured against
fills the face at the plane.

Shading the opening instead of capping it does not survive contact with an infill. Painting
every back face flat, on the reasoning that the nearest surviving surface inside an opening
is always a back face, gives a face with no depth of its own: it lands wherever the far
wall happens to be, and anything nearer draws over it. On a hollowed model with a Hive
infill, the lattice paints its own cells straight through the face.

ADR 0061 assumed a stencil plane was not available, because the viewport's depth format is
chosen by eframe. It is available. `egui_wgpu::depth_format_from_bits` maps `(24, 8)` to
`Depth24PlusStencil8`, `eframe::NativeOptions` carries a `stencil_buffer` field beside
`depth_buffer`, and `egui-wgpu`'s render pass attaches the stencil aspect with a
clear-to-zero load whenever the format it was given has one. The stencil was one
`NativeOptions` field away.

Capping a clipped solid with the stencil buffer is a solved problem, described in the
OpenGL advanced rendering course notes: embed a capping polygon in the clipping plane and
trim it with a stencil count of the crossings between the eye and that plane.

## Decision

The cut is capped by the stencil plane, in three draws inside egui's own render pass:

1. **Count the crossings.** Every solid on the plate is drawn again with colour writes
   off, depth writes off and depth comparison `Always`, keeping only the fragments the cut
   took away — `world_z > section`. Back faces count up, front faces count down,
   `IncrementWrap` and `DecrementWrap`. A ray that left as many solids as it entered
   before it reached the plane reads zero; one that is still inside a solid does not.
2. **Draw the models,** cut at the plane, as ADR 0061 has them.
3. **Fill the face.** One quad lying in the cutting plane, sized to the plate and
   everything on it, drawn with the stencil test `NotEqual` against a reference of zero
   and the ordinary depth test and depth write. The stencil trims it to exactly the
   material the plane runs through, and its depth hides everything behind it.

The window asks eframe for `depth_buffer: 24` and `stencil_buffer: 8`, so the viewport's
depth format becomes `Depth24PlusStencil8`.

What is counted is the same mesh that is drawn, cavity and lattice included. A hollowed
model's cavity is wound inward, so it subtracts itself from the crossing count exactly as
it does from the rasteriser's non-zero fill rule (ADR 0059): the wall caps, the hollow does
not, and the infill under the cut stays visible through the openings the cap leaves. Those
inside surfaces keep their own lighting and are washed 35% towards the cap tone, so that
inside reads as inside.

ADR 0061's rail, its one height in the globals uniform and its one comparison per fragment
all stand. What changes is its stated consequence that the cut has no cap.

## Consequences

The face is a real face: planar, at the plane's own depth, trimmed to the material and
sorted against everything else in the scene. Nothing inside a cut model can paint through
it, whatever the infill pattern or the support layout, and two solids cut at the same
height show one continuous face.

A cut now costs one extra pass over the scene's triangles per frame, with no colour and no
depth writes. At the 200k-triangle sizes this application already draws whole, that is a
vertex-bound pass over geometry already resident on the GPU; nothing is uploaded twice,
because the crossing pass draws the very meshes the model pass drew, from the same cache.
The instance buffer carries one extra instance per solid.

The count is only as good as the mesh. An open mesh, or one whose faces are inverted after
repair, leaves crossings unpaired, and the cap is then filled where it should not be or
left open where it should not be. That is the same class of defect the Scene panel already
reports, and the window still draws the models themselves whatever the count says.

Supports are counted too, so a cut through a column is capped like anything else; a hollow
blocker is not, because it is a marker rather than material.

The signal to reopen this is a scene whose depth complexity outgrows the stencil: eight
bits wrap at 256 crossings on one ray, which no plausible plate reaches, and `Wrap` rather
than `Clamp` keeps the arithmetic honest right up to that point.

## Alternatives considered

### Shade every back face as the plane, and give it the plane's depth with `frag_depth`

The cheap version of the whole idea: no extra pass, no stencil, one branch in the fragment
shader painting back faces flat, and the fragment writing the depth of the point where its
view ray crosses the plane so that the face is planar and hides the lattice. It caps every
ray that ever meets a back face, including rays that pass beside the model entirely and
cross the plane in empty air in front of it, so the cut paints over the part it was
cutting. Writing `frag_depth` also disables early depth testing for the model pass, the one
pass whose cost follows the triangle count. Without the depth write it is cheaper still and
wrong in a second way: an infill draws through the face, which is the defect this ADR
exists to fix.

### Slice the mesh at the cut height and mesh the contour into a face

`core-slicer` already produces closed contours at any height. Filling them needs a non-zero
polygon fill, which on the GPU is the stencil again, and a re-slice on every frame the
slider moves — a mesh-wide plane pass at 200k triangles for a control the user scrubs.

### The option that won, and what it costs

The stencil. It adds a pass over every solid on the plate for every cut frame, a second
instance per object, two more pipelines and a dependency on a buffer egui allocates for us
— if egui ever stops attaching the stencil aspect, the cap goes away and the pipelines
fail validation rather than degrading. It also trusts the mesh's winding, which an imported
model is not obliged to deserve. What it buys is the correct answer rather than a trick
that reads correctly only until something stands behind the plane.

Sources: the OpenGL advanced rendering course notes, "Capping Clipped Solids with the
Stencil Buffer", <https://www.opengl.org/archives/resources/code/samples/advanced/advanced97/notes/node10.html>.

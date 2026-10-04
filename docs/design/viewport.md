# The viewport

How `encrust-app` draws the plate and the models on it, and how the mouse orbits, selects
and drags. Decisions: ADR 0014, 0015, 0016, 0017, 0036, 0062, 0108, 0184.

## Coordinates

Plate millimetres, Z up, origin at the front left corner, so a model on the plate has
`z >= 0` inside `0..x_mm` by `0..y_mm`. This is the frame `core-raster` maps onto the
panel, so a model that looks placed is placed. X and Y are drawn along the two plate edges
in red and green, the grid is one line per centimetre, and a faint wireframe box marks the
top of the build volume. Under the plate is the build platform: a lipped deck and the arm
it hangs from, generated from the plate's own extents and painted last and translucent
with depth writes off, so what stands on the plate is never hidden by it. The machine
cures downwards, so its structure is on the far side from the print and nothing it draws
is above `z = 0`. `Front` is painted on the deck's front lip: laid out by the font, mapped
from points onto the lip in millimetres and drawn in the 3D pass against egui's own font
atlas, so it takes the machine's perspective (ADR 0108).

## The camera

`OrbitCamera` is a target, a yaw, a pitch and a distance.

```
eye = target + distance * (cos(pitch) cos(yaw), cos(pitch) sin(yaw), sin(pitch))
```

Pitch is clamped to `±(π/2 - 0.01)`: at the poles the look-at basis is undefined, and
stopping short costs nothing visible. The view matrix is right-handed with Z up; the
projection is wgpu's `0..1` depth convention (`camera::rh::proj::directx`), not OpenGL's
`-1..1` — getting that wrong halves the depth range and shows as flicker between coplanar
surfaces. Near and far track the orbit distance at `distance/100` and `distance * 20`,
since a fixed tenth-millimetre near plane would spend the depth buffer on empty space.

`frame` puts the target at a box's centre and the distance at `radius / sin(fov_y / 2)`
for `radius` half the box diagonal: exact vertically, conservative horizontally at any
aspect wider than tall, which is every viewport panel in practice.

## Input

| Gesture | Effect |
|---|---|
| Left drag | Orbit, 0.009 rad per point |
| Right, middle or shift + left drag | Pan the target across the view plane |
| Scroll | Zoom, `exp(-delta * 0.0015)` on the distance |
| Click | Select the object under the cursor, or clear |
| Double click | Frame the plate's contents, or the plate when empty |

None fire while a gizmo handle is hovered or dragged. Zoom is exponential so a notch
changes the view by the same proportion at any distance and no amount of scrolling reaches
zero. Panning converts points to millimetres with
`2 * distance * tan(fov_y / 2) / viewport_height_points`, the world height of the view
plane over its height on screen, which keeps the same world point under the cursor.

## Picking

The cursor becomes clip coordinates (`x` -1 left to 1 right, `y` 1 top to -1 bottom) and
the inverse view-projection turns clip `z = 0` and `z = 1` back into plate coordinates —
the near plane is 0, not -1, because the projection is wgpu's.

Each object is tested with the ray moved into the object's own space, since the mesh is
millions of vertices and the ray is two; the hit comes back to plate coordinates before
objects are compared, so an object under a scale cannot win on a shorter local `t`. A
transform whose determinant is near zero has flattened its model and is skipped —
there is nothing left to click. Intersection is `core_geometry::ray` through the scene's
`Bvh`, so a click costs microseconds whatever the triangle count (ADR 0016, 0036).

## The gizmo

`transform-gizmo-egui` draws the handles with egui shapes after the wgpu callback and with
no depth test, so they are never hidden inside the model. It is two of the crate's gizmos on one
pivot, the arrows longer than the rings, since the crate draws both at one size; where
both catch a press the arrow wins. The bounds brackets are egui shapes too, cut into
pieces and each tested with a ray from the eye through the `Bvh`, so the model hides them. `gizmo.rs` is the only module
naming a type from that crate (ADR 0017). Two boundary details: the crate takes matrices
row by row where `glam` stores columns, and it works in `f64`, so a rotation comes back
drifted off unit length and is renormalised on the way in. The camera must ignore a drag
the gizmo is handling, and the viewport reads `is_focused()` before its own input, so the
answer is one frame old — which only matters on the frame the cursor crosses a handle,
since hovering focuses it.

## The frame

`ViewportCallback` is built during the UI pass while the scene is borrowed, carrying plain
data: the view-projection, the plate's lines and one `ModelDraw` per visible object. egui
calls it back when the app state is no longer borrowed. `prepare` borrows the resources
mutably and uploads the globals uniform (view-projection, light direction, section cut),
the plate lines into a buffer that grows and never shrinks, one instance row per object,
any mesh not already on the GPU keyed by its `Arc` address, and the draw list. `paint`
borrows immutably and replays it: the line pipeline once, then the model pipeline per
object, and the machine last.

## Vertex layouts

Models are a flat-shaded triangle list with no index buffer: each face becomes three
vertices carrying that face's normal, because sharing vertices would need averaged normals
and round off exactly the facets a sliceable model is made of.

| Buffer | Step | Contents |
|---|---|---|
| 0 | vertex | `position: vec3`, `normal: vec3` |
| 1 | instance | `model: mat4`, `normal: mat3`, `color: vec4` |

The machine has a vertex layout of its own — `position`, `normal`, `color` — because it
is one draw of one buffer rather than instances of a cached mesh, and a lit pass of its
own, because the model shader is the print's: it cuts at the section plane, subtracts
drain holes and washes exposure bands, none of which a printer wants. Its deck covers the
plate, and stays under the grid by being drawn after it and losing every depth tie. The
word on its lip is a third layout — `position`, `uv`, `color` — and the only pass with a
second bind group, holding the font atlas and its sampler.

The instance normal matrix is the inverse transpose of the model matrix's upper 3×3, so
normals stay perpendicular under a non-uniform scale; a scale with a zero axis is not
invertible and falls back to the identity. Back faces are not culled and are lit as if
they faced the camera, so an inverted face reads as the mesh defect the Scene panel
reports rather than as a rendering bug.

A mirror is a negative scale, which turns every triangle's winding on screen while the
mesh keeps its own. The model, relief and crossing-count pipelines are therefore each built
twice, counter-clockwise front faces and clockwise, and an instance whose matrix has a
negative determinant is drawn through the second. Otherwise a mirrored model would be lit
as its own inside and would count the wrong way into the stencil.

## The section cut

Models are drawn up to the height the section rail is parked at. The globals uniform
carries `section = (height_mm, on, 0, 0)` — a height alone could not say "no cut", since a
model below the plate is legal — and the model fragment shader discards anything whose
world `z` is above it. The plate and grid go through the line pipeline and are never cut.

The opened face is capped with the stencil plane, three draws in the viewport's own pass:

1. **Count the crossings.** Every solid is drawn again with colour and depth writes off
   and the depth test `Always`, keeping only what the cut took (`world_z > section`), so
   what is counted is the geometry between the eye and the plane. Back faces
   `IncrementWrap`, front faces `DecrementWrap`, so a ray that entered as many solids as
   it left reads zero.
2. **Draw the models,** cut at the plane.
3. **Fill the face.** One quad in the plane, wide enough to cover the plate, stencil test
   `NotEqual` against zero plus the ordinary depth test, so it is trimmed to exactly the
   material the plane runs through and hides what is behind it.

What is counted is the mesh that is drawn, cavity and lattice included: a hollowed model's
cavity is wound inward, so it subtracts itself from the crossing count exactly as it does
from the fill rule — the wall caps, the hollow does not, and the infill stays visible
through the openings. Inside surfaces are lit as if facing the camera and washed 35%
towards the cap tone so inside reads as inside (ADR 0062).

Exposure bands are washed on in the same pass, 45% of the band's own tint over the
fragment's height, walked backwards so the last band wins as it does when the file is
written. Nothing is washed below the height the bottom block reaches, because a band has
no effect there (ADR 0090). Eight bands fit in the globals; the ninth is left untinted.

The stencil is the viewport's own: the scene is drawn into a window-sized colour plane and
a `Depth24PlusStencil8` plane in `render::target`, and one triangle copies the colour into
egui's pass, the same in a browser as at the desk (ADR 0184). Which height the
rail hands over depends on the mode, and `panels::section::cut_height` is the one place
that decides.

## The Cut tool's plane

The plane is traced on the model rather than drawn in the air. The globals carry it as
`cut_plane = (normal, offset_mm)` with the selected model's box in `cut_low`/`cut_high`,
and the model fragment shader darkens whatever lies within a pixel and a half of it,
measured with `fwidth` so the line keeps its width at any zoom. The derivative is taken
before any `discard`, as WGSL requires. The box keeps the line off every other model; an
outline painted over the frame shows the plane where it misses the model. It is
independent of the section rail.

## What is not tested

The rendered pixels and the gizmo's interaction. Everything above those boundaries —
camera matrices, framing, grid geometry, vertex expansion, normal matrices, ray casting,
picking, the gizmo's conversions — has headless unit tests. Asserting on the draw would
mean an image-comparison harness and a GPU in CI; asserting on a drag would mean feeding
synthetic pointer events through egui to test a third-party crate's arithmetic.

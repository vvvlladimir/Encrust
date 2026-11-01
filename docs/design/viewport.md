# The viewport

How `encrust-app` draws the plate and the models on it, and how the mouse orbits, selects
and drags. Decisions: ADR 0014, 0015, 0016, 0017, 0036, 0062, 0184, 0219.

## Coordinates

Plate millimetres, Z up, origin at the front left corner, so a model on the plate has
`z >= 0` inside `0..x_mm` by `0..y_mm`. This is the frame `core-raster` maps onto the
panel, so a model that looks placed is placed. The viewport stands on a flat `sunken`
backdrop, and the plate is one surface, with no model of the machine under it: a fill of
`plate` a shade off the backdrop with the grid drawn into it by the same fragment shader,
a line every centimetre and a stronger one every five. Each line is measured in pixels with
`fwidth`, so it keeps its width at any zoom and is smoothed over its edge, and the
centimetre lines fade out once they close to within a few pixels of each other. Drawn into
the surface, the grid can never fight it for depth. The surface is drawn first and takes no
depth, so the lines over it never tie with it and a model seen through still shows over it.
Three arrows 30 mm long start at
the origin corner, X and Y along the plate's edges, X red, Y green and Z blue, their letters
drawn over the 3D pass so they face the camera; they are the first lines drawn, so where
they run along the outline they win the depth tie. The plate's outline and the wireframe of
the build volume are translucent. While the section cuts, the plate's
outline is drawn again at its height, dashed 3 mm on and 2 mm off. `Front` lies in a 12 mm
band in front of the plate: laid out by the font, mapped from points onto the plate's plane
in millimetres and drawn in the 3D pass against egui's own font atlas, so it takes the
plate's perspective.

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

Faces of different meshes lying in one plane — a pad on a raft, two groups' pads standing
in each other — would still take that plane by turns. Each support group is drawn with
its depth taken a ten-thousandth of the way nearer the eye than the next, lowest group
nearest, so the same face wins every frame (`GROUP_PULL`); only depth moves, never where
a fragment is lit or cut. A section is drawn a micron inside the height it cuts at, so a
face level with the cut — a raft's top, a step's floor — is cut away under the cap
rather than fighting it (`SECTION_INSET_MM`).

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

A tool that puts something down with a click — Place, Hollow, Drain — draws what that
click would add under the resting cursor, see-through and never capped: a `Ghost`, built
by the same `core-volume` and `core-supports` calls the click makes. A body is drawn as it
would stand; a hole is cut into its model the way a drilled one is. Nothing shows while a
button is held, with alt (which takes away), or over a model the click would only pick.

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

A section cut shortens the ray to the heights it leaves drawn, so nothing it took away is
hit. A ray whose first face past the cut is one it leaves the material through crossed the
cut inside a model, where the cap is drawn: no face for a surface tool, but the model for a
selection and a point on the cut for a Keep solid blocker (`pick_material`).

## The gizmo

`transform-gizmo-egui` draws the handles with egui shapes after the wgpu callback and with
no depth test, so they are never hidden inside the model. It is two of the crate's gizmos on one
pivot, the arrows longer than the rings, since the crate draws both at one size; where
both catch a press the arrow wins, and while one is dragged the other is not drawn. There is
no view ring. Shift keeps a drag to 1 mm and 15° steps, and the rings show the steps as ticks.
A drag shows its amount by the cursor and, for a move, a dot where the pivot started and the
arrow's line through it (`gizmo_drag.rs`); the size labels step off the rings' disc. The bounds brackets are egui shapes too, cut into
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
mutably and uploads the globals uniform (view-projection, lights, section cut, shadow),
the plate lines into a buffer that grows and never shrinks, one instance row per object,
any mesh not already on the GPU keyed by its `Arc` address, and the draw list. `paint`
borrows immutably and replays it: the line pipeline once, then the model pipeline per
object, the word, then the shadow, before the caps and the markers.

Each model pipeline is built twice from the shader's overridable constants. The opaque
form writes without blending and has no `discard`: a tiling card hides a fragment only
behind such a draw, and a desktop card tests depth before it shades only when nothing
discards. The clipped form blends and discards what the section, a drain cut or a pocket
takes away. A model is drawn opaque when its colour has full coverage, it is a surface
rather than a volume, and the frame has no section or drain cut (`Globals::clips_models`).
The opaque models go first, then the clipped ones. `LOOK` also fixes which light a
pipeline is under, so the working view's pipeline carries no studio code and its
registers, and the studio's is built again for a moving camera (see the key's shadow). The
x-ray and the clear resin keep one form each.

## Vertex layouts

Models are drawn indexed and flat-shaded. Averaged normals would round off exactly the
facets a sliceable model is made of, so the normal is read `flat`, off a face's first
corner alone, and every face is turned to start on a corner no other face starts on; the
other two are shared. GL reads a flat input off the last corner instead, and naga cannot
write `flat, first` for it, so on that backend every face is turned one step further. A closed mesh has about half as many vertices as faces, so it lays
out one vertex a face plus three indices: 36 bytes a face against the 72 a corner per face
took. The normal is not read off the screen-space derivatives of the position instead,
which would halve it again: on the machine this was built on the same frame came out
different from one run to the next.

Supports are a tube of few sides, not a sliceable model's facets, so an instance may ask to be
shaded smooth (ADR 0341): its mesh goes up with every corner carrying the faces round it
weighted by their angle there, apart across an edge turning more than 50 degrees and shared
by every face that smooth edges join round it (`core_geometry::smooth_normals`, linear in
the faces), and the fragment reads that normal blended across the face instead of the flat one.
The normals come built with the mesh (`core_supports::Shaded`), off the drawing thread.

| Buffer | Step | Contents |
|---|---|---|
| 0 | vertex | `position: vec3`, `normal: vec3` |
| 1 | instance | `model: mat4`, `normal: mat3`, `color: vec4` |

The word in front of the plate is a layout of its own — `position`, `uv`, `color` — with a
second bind group holding the font atlas and its sampler.

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

Models are drawn between the strip's two handles: up to the height the top one is parked
at, and down to the floor the bottom one is. The globals uniform carries
`section = (top_mm, on, bottom_mm, on)` — a height alone could not say "no cut", since a
model below the plate is legal — and the model fragment shader discards anything whose
world `z` is above the top or under the floor. The plate and grid go through the line pipeline and are never cut.

The opened face is capped with the stencil plane, three draws in the viewport's own pass:

1. **Count the crossings.** Every solid is drawn again with colour and depth writes off
   and the depth test `Always`, keeping only what the cut took (`world_z > section`), so
   what is counted is the geometry between the eye and the plane. Back faces
   `IncrementWrap`, front faces `DecrementWrap`, so a ray that entered as many solids as
   it left reads zero. That is what stands in front of the plane only for an eye above
   it, so the top is capped only from above, the floor only from below.
2. **Draw the models,** cut at the plane.
3. **Fill the face.** One quad in the plane, wide enough to cover the plate, stencil test
   `NotEqual` against zero plus the ordinary depth test, so it is trimmed to exactly the
   material the plane runs through and hides what is behind it.

The floor is capped the same way after the top, once a screen triangle has put the stencil
back to zero: the count keeps what stands under the floor, which is the geometry between the
eye and the plane when the floor is looked at from below — the only side it can be seen
from, since from above the slab over it hides it.

## The inside of a cut

A drain hole or a channel never cuts the mesh: the fragment shader drops what stands inside
the tube (ADR 0073) and the fill rule subtracts the same body when the layer is rasterised.
What closes the opening is the cut's own body, drawn with the stencil the same way the cap
is, four draws per object that carries cuts and before anything else in the pass (ADR 0201):

1. **Lay the candidate.** The bodies, one copy, into the depth plane with no colour. They
   are wound inward, so the face that survives culling is the far wall of the tube — the
   one a hole is looked into.
2. **Count the material.** The object itself with the counting stencil, depth-tested `Less`
   against that candidate and writing neither colour nor depth: how much material stands
   between the eye and the cut's own surface. The section takes nothing off it, since
   whether the surface stands in material is a question about the whole solid.
3. **Draw the surface** where the count ran negative, with the model's own shading.
4. **Wipe the depth** where it did not, writing a depth of 1, so a tube hanging in the air
   beside the model leaves no ghost over what stands behind it.

The stencil goes back to zero between objects, and the section cap counts from zero after
them. Nothing meshes the surface of a hole, so there is no curve to approximate and no
crack where one would have been.

What is counted is the mesh that is drawn, cavity and lattice included: a hollowed model's
cavity is wound inward, so it subtracts itself from the crossing count exactly as it does
from the fill rule — the wall caps, the hollow does not, and the infill stays visible
through the openings. A model that is not sound counts nothing at all: crossings only
describe an inside on a closed surface, and the ones a hole swallows would paint the cap
out into the air beside the model (ADR 0195). Inside surfaces are lit as if facing the
camera and washed 35% towards the cap tone so inside reads as inside (ADR 0062).

The cap writes no depth. What a tool puts on a model — a blocker, a channel's points, a
ghost — is drawn after it, so the cut is a window onto what stands in the material, while
the model's own faces still hide it from every other side. A marker is translucent, so it
is drawn far half first and near half over it; where the cut opens one, its far half is lit
as the flat face the cut leaves, so a cut ball reads as a disc rather than a bowl.

Exposure bands are washed on in the same pass, 45% of the band's own tint over the
fragment's height, walked backwards so the last band wins as it does when the file is
written. Nothing is washed below the height the bottom block reaches, because a band has
no effect there (ADR 0090). Eight bands fit in the globals; the ninth is left untinted.

The stencil is the viewport's own: the scene is drawn into a window-sized colour plane and
a `Depth24PlusStencil8` plane in `render::target`, and one triangle copies the colour into
egui's pass, the same in a browser as at the desk (ADR 0184). Which height the
rail hands over depends on the mode, and `panels::section::cut_height` and `floor_height`
are the one place that decides. Preview cuts at the plane the shown layer's mask is
sampled at, not at the layer's top the readout states: the middle of the band with one
sample, the highest of them with several, so the section shows nothing the mask lacks.
Cut at the top, a part beginning in the upper half of a layer showed a layer early.

## Light

Every lit pass shares one `lit` function, in the gamma space the target is written in
(see `ui-design-system.md`), from the tokens in `theme::Scene::light`:

```
colour = base * (floor + sky_share * mix(ground, sky, n.z / 2 + 1/2)
                 + key_share * key * max(n · key, 0) + fill_share * fill * max(n · fill, 0))
       + specular_share * key * max(n · h, 0)^24
```

The ambient is a hemisphere, cool from above and warm-dark from below, over a floor of
0.26, so a face turned from both lights still reads as resin rather than as a hole. The key
comes from above the front left and the fill, weak and warm, from the back right. The
highlight is Blinn-Phong with `h` halfway between the key and the eye, small and broad as on
a matt resin, scaled by the fragment's alpha so a translucent surface stays premultiplied.
The shares are shader constants; the colours and directions are tokens.

A surface leaning past the overhang angle is filled with the overhang colour at 0.85, so
where the need for a support ends is one line (ADR 0245). Whatever stands past the build
volume, or in the plate's inset where the printer has one (ADR 0371, the plate darker there), is hatched rather than filled: stripes 3.4 mm apart along `x + y - z`, so they run
diagonally on any face, smoothed over a pixel with `fwidth` and evened out to half where
they are closer together than pixels. Between the stripes the mark keeps a light wash, so
a part narrower than the pitch still shows.

## The contact shadow

The models darken the plate under them with a shadow baked into a 512 by 512 one-channel
texture over the plate. The silhouette pass lays every model flat onto
it from above, both faces and no depth, each fragment as dark as its height leaves it —
`1 - smoothstep(0, 30 mm, z)` — and a blend of `Max`, so the surface nearest the plate
over a texel decides it. A volume casts nothing, a translucent marker as much as it covers,
and nothing under the plate shades its top. One blur pass, a 7 by 7 Gaussian 1.5 texels
apart, softens it into the second texture.

The bake runs only when what casts it changes: `prepare` hashes the plate's rectangle and
each model's mesh address, faces and instance row, which costs the same for a model of
millions of triangles as for one of twelve, and `draw` bakes again only when that number
moved. An orbit, a zoom, a scrub of the layer strip or the x-ray bakes nothing; a drag of
the gizmo bakes every frame it moves a model.

Every frame a quad over the same rectangle at `z = 0` reads the blurred texture and lays
black at `shadow`'s alpha times what it reads, over the backdrop the viewport is presented
on. It is drawn after the models, with the depth test on and depth writes off, so whatever
stands in front of the plate keeps it off and the grid lines stay crisp, and before the
caps: they take no depth, and a model's floor lies in the plate, where the shadow would tie
with it and show through a cap in stripes. From under the plate it discards.

## The view cube

The cube at the viewport's top right is egui shapes in an `Area` of its own, so the camera
never reads a press on it. It is drawn without perspective, turned by the camera's view
matrix alone. Each face is cut into nine cells at half its half-width: the middle stands
for the face, the bands for the twelve edges and the corners for the eight corners, each
cell's view the sum of the normals it touches. The cell under the pointer lights up with
every other cell of the same view. A click swings the camera round its target to look
from there with `OrbitCamera::seen_from`; straight up or down keeps the yaw, which is
undefined there. Each face's name is laid out by the font and its triangles carried onto
the face's plane before they are projected, so it lies on the face and foreshortens with
it. A face turned less than 0.08 towards the camera is not drawn: seen edge on it would
be a hairline down the cube's side. Under the cube, while the pointer is over it, Home
swings back to the view an empty window opens with; its room is kept whether it shows or
not, so it does not vanish on the way down to it.

The swing is a `CameraTurn` on `View`, eased with a smoothstep and taking the short way
round in yaw. Any drag or scroll of the camera ends it where it stands.

## Marks and flying to a spot

What a tool points at on a model is a `highlight::Highlight`: faces as patches of their own
stood `LIFT_MM` off the surface, edges as wide lines — a quad per edge the shader lays
across the screen `theme::MARK_LINE_W` points wide, its depth pulled toward the eye so the
faces beside the edge never cover it — each mark in its own colour and drawn in
the order listed, so a mark over another stands further off or comes later. The Repair
tool's findings are the first (ADR 0355). A Check row cuts the view at its layer, in
Prepare by moving the section there, and rings each issue on the cut, as wide as a disc of
its area and at least 0.5 mm; the rings go once the cut or a model moves. The camera looks down on
them, seen through the cut, so its `Spot` faces +Z.

A place to look at is a `Spot`: centre, radius and the side it faces. `OrbitCamera::focusing`
frames it so the sphere of `radius / 0.35`, at least 3 mm, touches the frustum along the
narrower side: `r / sin(half angle)` back, `r / tan` without perspective, where the view's
half height at the target is what the distance sets. The defect fills about a third of the
view and the rest shows what it lies in. It is seen from its own side unless the view
already faces it within 45°, when only the target and distance move. A side
`pick::sight_blocked` finds drawn in front of — a model between the cuts, the cap across
its material, its built supports — is passed over for rings tilted 30° then 60°
round it, the one above first, then the current view if that is still outside; never from
behind, which is from inside the model. With every side blocked it takes its own, and the
x-ray shows through. A spot with no side of its own, a branching rim or faces drawn both
ways, tries the current view and then the axes and diagonals nearest to it. A hole's side
is its loop's, as its patch would face, or for a branching rim the faces along it summed.

Every swing, the view cube's included, moves the target and the distance along the path
van Wijk and Nuij derive ("Smooth and efficient zooming and panning", 2003) with ρ = √2: a
long way pulls back, crosses and closes in, a short one hardly zooms, and zooming in place
scales evenly. Its length S sets a flight's time, 0.35 s a unit between 0.35 and 1.2 s; the
view cube keeps its own 0.35 s. The angles ease along the same smoothstep, and the two ends
are the views themselves, not the path's rounding of them.

## Without perspective

`OrbitCamera::orthographic` draws the view with parallel lines staying parallel; the view
tools and the View menu switch it. The orthographic view is as tall as the perspective one
is at the target, `2 · distance · tan(fov / 2)`, so switching keeps what stands there the
same size and zoom and pan work unchanged. Its near plane stands ten distances behind the
eye, so a model the eye is zoomed into is still drawn whole. Picking unprojects through the
same matrix and needs nothing else; what asks where a line of sight starts — whether a
bracket is hidden, the plane a dragged tip moves in — asks `sight_to`, which is the eye in
perspective and a point far back along the view axis without it.

## Seeing through the models

The x-ray is a view of its own, not a tool: the view card and the View menu toggle it and
nothing else ever does (ADR 0198). The models are drawn by the same shader through a
second pipeline with no depth at all — `depth_write` off and `depth_compare`
`Always` — so nothing hides anything and the wall, the cavity behind it, the lattice
standing in it and the drain holes all paint, each adding its own wash. The more material
a ray crosses, the brighter it reads.

How much of itself a surface keeps is `theme::SEEN_THROUGH` on an edge turned away from
the camera and the shader's `XRAY_FLOOR` of that where it faces the camera, so a wall in
front of a cavity is glass while every rim draws its own line. The factor scales the shaded
colour and the alpha together, which keeps the fragment premultiplied for the blend and
washes the overhang mark and the inside wash down with the surface rather than the token
they were mixed into.

An instance says whether it is a **surface** or a **volume**. A surface is what the
paragraphs above describe. A volume is the space between the two walls that bound it — the
red a cavity holding resin is painted with — so no light shades it, the inside wash does
not lighten its far wall, and the x-ray does not wash it down: both walls lay the token
down as it is, and what the eye reads is the token twice over. That is why `trapped` is far
more translucent than it looks: head-on it is laid twice, where a surface in the same place
would be down to `XRAY_FLOOR` of itself. Volumes are gathered after every model, so a
second model standing in front cannot wash one out.

Everything else applies to the flat pass alone: the plate, the section cap and
a model drawn with its own texture are unchanged, so a relief still shows what it would
press in.

That red is kept to the pockets the check found: the globals carry up to `MAX_POCKETS`
boxes in plate millimetres, and a volume fragment standing in none of them is discarded.
A cavity is one mesh however many pockets stand in it, so without that test draining one
of three would change nothing on screen (ADR 0200).

That red is the shell's own cavity faces drawn a second time, not a mesh of its own: a
`ModelDraw` names a face range, and `pieces` breaks a cached mesh at the ends of every
range drawn from it as well as at the card's ceiling, so part of a mesh is a whole number
of pieces and nothing is uploaded twice (ADR 0070, 0190). Nothing is culled, so the near
wall of the cavity and its far wall both paint, and the space between them reads as a
volume rather than as an outline.

A shell reaches the card frames before the drainage check says whether its cavity is to be
painted, so the draw of the whole shell *declares* that range as well — `ModelDraw::breaks`
— and the pieces are cut for it from the first upload. The cache compares the breaks it
holds against the ones asked for and uploads again when they differ, which is the net under
that: without it the later frame asks for a range no piece lines up with and paints
nothing at all.

## As printed

The render view is a view of its own, like the x-ray: the view tools and the View menu
toggle it (ADR 0390). Overhangs, cuts and bands still wash over it; picking, the contact
shadow and the levels are those of the working view. `globals.look` carries it: the layer
height, 1.0 for the studio, the resin's absorption per mm and the eye's pivot distance.

- **Resin.** Every model and support instance takes the resin's `details.color` in place of
  the theme's model, selection and support tints: the selection tint is not drawn.
- **Studio.** `studio` replaces `lit`. The room every surface sees round itself is
  `studio_sky`: the cove fading up into a pale sky, and three softboxes — the key, one
  overhead, a warm fill opposite the key — each a bright disc with a soft rim. It is baked
  once, and again only when the theme's lights change, into a 256 × 128 equirectangular
  RGBA16Float map of six mips, mip *i* prefiltered for roughness *i*/5 by 128 GGX
  importance samples on Hammersley's sequence, the normal, the eye and the reflection taken
  as one: Karis's split sum (2013). A surface reads its reflection from the mip its
  roughness picks, scaled by Karis's analytic fit of the environment BRDF (2014) in place
  of the split sum's table, and dimmed where the key's shadow falls; it reads the roughest
  mip about its normal as the light the room gives it, and adds the key's own light,
  wrapped round its edge, where the shadow lets it. A resin is 0.32 rough, 0.55 where its
  layers groove it; the props carry their own. All of it is summed in linear light
  (`radiance`: every colour token and resin decoded from gamma first) and handed to
  `filmic`, AgX — Rec. 2020 inset, log2 exposure from −12.47 to +4.03 EV, the fitted
  sigmoid polynomial, outset — with a milder punchy look, the curve raised to 1.15, before
  it goes out in the target's gamma. AgX was chosen over ACES for leaving a saturated
  resin's hue alone as it brightens. The backdrop goes through the same curve; the working
  view does not.
- **Layers.** `stepped` turns a surface's normal towards the stair a printer cuts it into:
  within each layer the share `|n.z| / (|n.xy| + |n.z|)` is a flat tread and the rest a
  vertical riser, so a wall shows only the lip line each layer leaves, a gentle slope the
  steps, a flat top none. The lip darkens by `LAYER_LIP` over a pixel. Both fade to the
  smooth surface between `LAYER_FADE_FROM` and `LAYER_FADE_TO` layers a pixel, under about
  four pixels a layer, where a stair would only beat against the pixel grid. The overhang
  test still reads the smooth normal.
  Layers too fine to see still groove the surface, and the grooves stretch its highlight
  across them, a satin sheen. As glTF's `KHR_materials_anisotropy` does it, reflections are
  taken about a normal bent towards the one facing the eye in the plane across the layers,
  by the layers' roughness (below) times `SHEEN_BEND`; and the key adds Ward's anisotropic
  lobe, 0.12 wide along the layers and up to 0.67 across them.
- **Thin parts glow.** At rest the thickness pass below runs for any resin, and the models
  are drawn by `glow_fragment` against it: light through a thin part, after Barré-Brisebois
  and Bouchard (GDC 2011) — the key seen through the part, bent towards its
  normal by 0.2, narrowed to the power of 4, over a little from all round — carried through
  `exp(−d / 2.5 mm)` of a part `d` thick, so an ear, a finger or an edge lights up in the
  resin's colour with the key behind it. The thickness stands in for the paper's baked
  local thickness. An open mesh, which measures none, does not glow.
- **Clear resin.** Modelled after glTF's `KHR_materials_volume`, with
  the surface the layers leave. A resin's opacity, picked with its colour, sets an
  attenuation distance *D* from 1 mm, barely clear, to 300 mm, water-clear
  (`1 · 300^clarity`). Two passes come before the frame. `measure_thickness` draws every
  surface of the clear models, nothing culled and no depth, adding its distance from the
  eye less the pivot's into an R16Float plane: a front face subtracts and a back face adds,
  so the plane holds the resin each ray crosses, a hollow's cavity taken out. The exit pass
  keeps each pixel's nearest back face, its outward normal and distance, in an RGBA16Float
  plane: where the light inside first leaves the part. The scene pass then draws all else,
  keeping its depth, and ends by laying every face of the clear models into it faintly —
  the studio's reflection off it, brightest at a grazing angle, over a trace of the resin's
  colour — so walls inside, a hollow's, a lattice's, the far side, read through the part.
  Its colour plane is copied out, and a second pass draws the models' front faces by
  `clear_fragment` over that copy.

  Each fragment traces the light per channel, the resin's index 1.49, 1.50 and 1.52 for
  red, green and blue: bent into the part at the surface, carried to the exit face, bent
  out of it, or turned back inside past the critical angle, and carried as far again; it
  reads the copy where that lands. What stands behind is displaced, magnified and fringed.
  The layers decide the surface. Where they are a few pixels apart or more, the entry is
  the stair `stepped` cuts, so what is behind is broken into bands. Finer than that no
  picture resolves them and they become roughness, as a normal map is filtered into it: the
  risers and treads turn the light as far apart as they share the surface evenly,
  `2·√(t·(1−t))` for a tread share `t`, at least 0.3 on a wall for its lips, nothing on a
  flat top. The two faces' roughness add, and spread the light 0.35 rad a unit over the
  thickness crossed, across the layers only: the eight taps the copy is read through lie on
  an ellipse long up the surface's slope as it lands on screen, 1.5 px across it. Over the
  thickness *d* the light takes the resin's colour as `colour^(d/D)` and clouds over by
  `1 − e^(−d/D)`, within 400 mm even in the clearest resin as curing hazes it, and a rough
  surface scatters 0.28 of its roughness back as a white haze, all into the resin lit as an
  opaque one; Schlick's Fresnel shares it with the studio's reflection off the stepped
  surface. Where the copy holds nothing, the backdrop shows. Off under the x-ray, and
  while the camera moves, when the part is drawn opaque (ADR 0391).
- **Creases.** At rest — no level settling, so not while the camera moves — the copy pass
  shades creases from the depth plane, which is kept past the scene pass for it, by
  ground-truth ambient occlusion (Jimenez et al. 2016).
  `present.wgsl` rebuilds each pixel's point through the inverse projection and its normal
  from the shorter step to a neighbour, so a silhouette does not tear it; along three
  slices turned by interleaved-gradient noise, and a golden-ratio share of a turn more each
  frame of a picture at rest, it finds the horizon both ways in four steps each, spaced
  quadratically out to `REACH_MM` (4 mm) and at most 48 px, each sample's say fading over
  the outer 61.5 % of the reach; and integrates the light the horizons let in
  over the arc the normal projected into the slice sees, cosine-weighted. It only ever
  darkens, in linear light. The paper's spatio-temporal denoise is the picture at rest:
  sixteen jittered frames average its noise away, so it has no blur pass of its own.
- **The room.** The printer is put away: no plate, grid, outline or Front word. The scene
  pass starts with `backdrop_fragment` in `props.wgsl`, which turns each pixel back into a
  line of sight through the inverse view-projection and finds where it meets a
  photographer's infinity cove round the mat's middle: the table 3.05 mm under the plate's
  level out to 1.4 m, curving up over 0.7 m into an upright wall, so there is no horizon. The
  room inside is convex, so a ray leaves it once: a floor point inside the curve or a wall
  point above it, each met in closed form — a plane, an upright cylinder — is that exit.
  Any other ray meets the curve, marched eight steps from where it enters the curve's ring
  by its distance to the curve's circle, never past the wall's. Drawn unblended, the cove
  is hidden wherever an opaque model is drawn over it. The cove is lit by the studio's ambient and key in linear light,
  falls off a quarter towards the wall as a lit set's background does, takes the key's
  shadow and the mat's own on the table, and goes out through the same curve. It has no
  geometry. In the copy pass the frame's corners darken by up to 22 % from 45 % of the way
  out, as a lens vignettes.
- **The set.** A self-healing cutting mat lies under the models, its top 0.05 mm under the
  plate's level so the models' contact shadow never fights it, and things everyone knows
  the size of lie round them: in front, a 149.5 mm ballpoint and a 100 mm steel ruler; on
  the left, a one-euro coin (23.25 × 2.33 mm) and a 330 ml can (⌀ 66.2 × 115.2 mm); on the
  right, a banana swept along a 110 mm arc, about 180 mm long. `references::lay_out` places them 20 mm clear of
  the models' box seen from the front, each row on its own side so none can meet another,
  and picks the smallest mat sold, A3 to A0 either way round, that takes the whole set with
  a 30 mm margin, centred on it. Every mesh is built once from `core_geometry` primitives, a
  revolve or a sweep, with normals smoothed within 40°; the mat is a unit box its placement
  sizes, and the shader reads its size from `globals.mat`. Each is drawn as a marker, never
  picked, sliced or capped, its instance naming its material in the overhang slot as
  `−2 − material`; `prop_colour` paints it in its own millimetres under `finish`, the
  studio's light with a metal term: the mat's 10 mm grid, every fifth line heavier, with a
  45° line from each near corner and the centimetres printed along two edges in
  seven-segment figures; the can's lacquered wrap between bare metal of #151618, its lid
  sunk in a rolled rim under its tab, the whole can turned 30° so its print faces the models
  on its right; the banana yellowing into a brown stem, dark at its blossom end, streaked and
  freckled by smooth value noise; the coin's two metals inside its raised rim. Only the can's
  label is a picture.
- **The can's print.** The wrap carries `assets/props/can-label.jpg`, the one picture the set
  ships; any picture `image` decodes replaces it by swapping the file. `references/can_print.rs`
  sets it as high as the can's whole straight wall, 5 to 104.2 mm (`CAN_WALL_MM`), and as wide
  as it is in proportion, never past the girth, in the middle of a 2048 texel girth; each row
  of the gap behind fades from the picture's right edge round to its left, each edge the mean
  of its outer 1 %, so the wrap closes with no seam. A mip chain is averaged in linear light,
  the texels kept encoded, since `finish` linearises every colour it takes. It is drawn off
  the drawing thread when the window opens and bound beside the globals the first frame the
  render view stands the props, a dark texel standing in until then. A prop is shaded under
  its material's switch, where no derivative may be taken, so `can_print` picks its level
  from the pixel's footprint in millimetres.
- **The key's shadow.** Everything in the set, the models and the props and the mat, casts
  a shadow from the key light. `light.rs` draws their depth from the key, orthographic over
  the mat and 60 mm past it, into a 2048² map: an A2 mat is 0.3 mm a texel. It is drawn
  only when what casts it changes and the camera rests, as the contact shadow is, so an
  orbit costs nothing; a map just made is cleared to all lit. It is read as percentage-closer
  soft shadows (Fernando 2005), the key a softbox 4° across: twelve probes 12 mm
  round a point find what stands nearer the key, each against the depth the point's own
  surface would have there, so a slope the key falls across is not taken for something over
  it (receiver plane depth bias); the penumbra is the depth under the nearest blockers
  times the softbox's angle, at least 0.4 mm, and twelve compared taps over it give the
  light. A shadow is sharp where an object meets the mat and widens away from it. While the
  camera moves, when a penumbra is not looked at, the models and the backdrop are drawn by
  pipelines whose `LOOK` reads the map in one filtered tap, a hard edge. The map
  is orthographic, so its depth and its texels are millimetres by factors read off its
  matrix. Every surface under the studio's `finish`, and the table under the backdrop,
  reads it, and loses the key's diffuse and its reflection where it is shaded. Faces are
  pushed back by their slope as they are drawn and points lifted 0.4 mm off their surface
  as they are read, up to 2.4 mm as they turn from the key, so a lit surface never shades
  itself. A clear resin casts none: it lets the key through, and what grounds it is the
  contact shadow.

- **A picture at rest.** Once the camera stops, the render view sharpens over frames, by
  temporal accumulation with no reprojection: each frame is drawn a fraction of a pixel
  aside, by Halton (2, 3) as TAA jitters — the backdrop's line of sight with it — and
  averaged into an RGBA16Float plane by a constant blend of `1/(n+1)` for the n-th, a
  running mean the copy then shows. After sixteen frames the picture is held and nothing is
  drawn until it changes; the window asks for a frame at a time until then. What it is a
  picture of is hashed — the whole uniform, the lens, every mesh, face range and placement
  drawn, and every line, mark and label — so any change, not only the camera's, starts it
  again. Edges lose their steps, and the grain of the crease shading, the frosting and the
  soft shadows averages away.

- **Depth of field.** With the View menu's *Depth of field*, a picture at rest is seen
  through a lens: each of its frames is drawn from another point of the aperture, by
  Halton (5, 7) on the disc, and the view sheared back so the plane in focus stands still.
  Averaged over the frames, what stands off it blurs with true occlusion and no
  post-process. The models stay sharp: the plane lies at the harmonic mean of their nearest
  and farthest depth, where both blur alike, and the aperture opens to 1.6 % of the focus
  distance in radius but no wider than keeps that blur within half a point. Close to a
  model, its own depth leaves the lens nearly a pinhole and the set behind barely blurs;
  further out the set blurs more. Without a model the lens focuses on the point the camera
  turns about. It needs perspective; without, the lens is a pinhole.

On a WebGL canvas (`Backend::Gl`) neither the clear pass nor the creases run: the view draws
opaque and unshaded.

The planes only the render view reads go when it is turned off and come back the first
frame it is on again: at 4K the thickness, exit, its depth and the copy behind are some
150 MB, and the averaged plane 66 MB. An opaque resin reads only the thickness, so its exit
and copy are a texel each.

## The Cut tool's plane

The plane is traced on the model rather than drawn in the air. The globals carry it as
`cut_plane = (normal, offset_mm)` with the selected model's box in `cut_low`/`cut_high`,
and the model fragment shader darkens whatever lies within a pixel and a half of it,
measured with `fwidth` so the line keeps its width at any zoom. The derivative is taken
before any `discard`, as WGSL requires. The box keeps the line off every other model; an
outline painted over the frame shows the plane where it misses the model. It is
independent of the layer strip.

## What is not tested

The rendered pixels beyond what `render/offscreen.rs` asks of them, and the gizmo's and
the view cube's interaction. Everything above those boundaries — camera matrices, framing,
turns, grid geometry, the cube's cells, vertex expansion, normal matrices, ray casting,
picking, the gizmo's conversions — has headless unit tests. Asserting on the draw would
mean an image-comparison harness and a GPU in CI; asserting on a drag would mean feeding
synthetic pointer events through egui to test a third-party crate's arithmetic.

## Levels of a heavy model

A plate of sixteen copies of a million-face model is eighteen million faces a frame, which
one card draws in 6 ms and another in 30. So every mesh over `MIN_FACES` has a chain of
lighter levels, and each frame draws every model at the finest level the card can afford;
ADR 0259 holds why.

**The chain.** Each level is simplified from the one before to about half its faces, by
edge collapse, or where that stalls on a surface it may not tear, by merging corners on a
grid; it stops at `FEWEST_FACES`. A level's error, mm, is the sum of the simplifier's own
along the chain, or what the middles of a sample of its faces stand off the mesh, whichever
is more: the simplifier measures corners, and on a curved surface a face sinks in between
them by five times what it reports. A million faces take a second on a worker, the first
level half of it; the chain is keyed by the mesh's `Arc`, so copies share one.

**What the card can afford.** Where the device has timestamp queries the scene pass is
timed on the card itself and read back a few frames late. A least-squares line through the
passes, the older weighing less, tells the frame's own cost — its pixels, the plate — from
the cost of a face, and the budget is the faces left in 12 ms while the camera moves and
30 ms at rest. A pass dearer than the line foresees is believed at once and a cheaper one
over a second, so the budget falls at the first heavy frame and climbs back. Without such
a clock — a WebGL canvas — runs of slow frame intervals cut the moving budget instead,
nothing raises it, and a resting frame draws everything whole.

**Which level.** A level's error times the pixels a millimetre spans at the model's
nearest to the eye is what it shows on screen. `allot` starts every model whole and, while
the frame is over its budget, takes the step that shows least next. While the camera
moves no step past 4 px may be taken (ADR 0391), so a model in motion still reads as
itself and a frame past that is slower instead; at rest only those under half a pixel,
which cannot be seen, so a card with room to spare draws everything whole and one without
gives up only what is not seen. A picked model keeps a fifth of its faces at least. The
budget is learnt afresh when the render view is turned on or off, which costs more a face
and a frame, and while the camera moves that view draws a clear resin opaque. Once the camera stops, every model comes back a level a frame towards what
the rest allows, never coarser than it is, so detail sharpens over a few frames instead
of all at once.

A level may not be a cap's count or a cavity's faces: while a cut is capped every model is
drawn whole, and at rest a hollowed model is too, since the trapped resin it shows is drawn
from its own mesh's face ranges. A hole's inside is drawn against the level drawn, so it
still meets the surface around it. The shadow is not baked while levels still change.

A model moved — dragged by the gizmo or the pointer, or typed into the inspector — counts
as motion for the same 0.3 s as the camera: every model is drawn at the moving levels, the
one in hand keeping its fifth. Unlike a turn of the camera, its shadow and the key light's
map are baked again each frame, from the light levels, or they would stay where it was. A
model added or taken away, or another plate shown, moves nothing. The budget learns only
from the camera's motion, whose frames bake no shadow.

## Measuring a frame

`render/frame_time.rs` writes what the viewport's frames cost under the `frame_time` log
target, at debug, so it stays silent unless asked for:

```sh
RUST_LOG=info,frame_time=debug cargo run --release -p encrust-app -- model.stl 2> frames.log
```

Once a second while frames keep coming it writes a summary, and any frame more than 50 ms
after the one before it on its own line. A gap of a second or more is the window resting,
since it draws only when something changes, and is not counted.

| Field | What it is |
|---|---|
| `interval_ms` | One frame to the next: what the eye sees, the card included |
| `ui_ms` | The window's pass over its panels, CPU |
| `prepare_ms` | Preparing and recording the viewport's passes, uploads included, CPU |
| `drawn_faces`, `draws` | Triangles and mesh draws sent to the card, every pass counted |
| `uploaded_faces` | Triangles uploaded because a mesh was new to the card |
| `shadow_baked` | Whether the models were drawn once more into the contact shadow |
| `card_ms` | The card's own time over the latest scene pass read back, a few frames late |

An interval far above `ui_ms + prepare_ms` is time spent on the card or in presenting it;
one close to them is the CPU's. `card_ms` tells the card from a hand that paused under a
second: an interval far above it was spent waiting.

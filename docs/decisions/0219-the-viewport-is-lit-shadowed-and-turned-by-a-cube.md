# 0219. Light the viewport by a sky, a key and a fill, shadow it by contact, and turn it by a cube

- **Status:** Accepted
- **Date:** 2026-10-10

## Context

The v2 design gives the viewport a flat backdrop, a hemisphere of ambient light with a key
and a warm fill, a soft shadow on the plate under the models, hatched marks instead of
solid ones, a 10 and 50 mm grid with the axes in the corner, a dashed frame at the section's
height, a view cube, a choice of perspective, and no model of the machine under the plate.
The viewport had a translucent deck and arm drawn under the plate (ADR 0108), one
directional light and a flat ambient spelled in the shader, solid overhang and
out-of-volume colours, red and green plate edges for axes, and nothing that turns the
camera to a named view. Rule 7 of the architecture asks that
cost follow the work: a model of 865 000 triangles is drawn every frame already, and a
shadow that costs another pass over it on every frame is the cost to avoid.

## Decision

- **Light.** One `lit` function in `shader.wgsl` serves the model and relief passes: a
  sky-to-ground hemisphere over a floor, a key from above the front left, a weak warm fill
  from the back right, and a small Blinn-Phong highlight. Colours and directions
  are tokens in `theme::Scene::light`; the shares stay shader constants.
- **Hatching.** Overhangs and what stands past the build volume are diagonal stripes in
  plate millimetres, 3.4 mm apart, with a light wash between them.
- **Contact shadow.** Every model is laid flat from above into a 512 by 512 `R8Unorm`
  texture over the plate with a `Max` blend, fading over 30 mm of height, and blurred once.
  It is baked again only when a hash of the plate and each model's mesh, faces and
  placement changes, and laid on the plate as black, last.
- **View cube.** egui shapes in an `Area` at the viewport's top right, 26 cells for the
  faces, edges and corners, each face's name a text mesh carried onto its plane, and Home
  under it while the pointer is over the cube. A click starts a `CameraTurn`, 0.35 s eased,
  short way round in yaw; any drag or scroll ends it.
- **Perspective.** `OrbitCamera::orthographic` draws the view without perspective, as tall
  as the perspective view is at the target, switched from the view tools and the View menu.
- **No machine.** The deck, the arm and the knob are gone; the plate is its lines over a
  fill a shade off the backdrop, and `Front` lies alone in a band in front of it. This
  supersedes ADR 0108.
- **Table.** One plate surface with its grid drawn into it by the shader, every 10 mm and
  stronger every 50, a translucent plate outline and
  volume, X, Y and Z arrows from the plate's origin corner along its edges with letters
  drawn over the 3D pass, a dashed plate outline at the section's height, and a `sunken`
  backdrop.
- The selection stays a fill colour; an outline is left for later.

## Consequences

- An orbit, a zoom or a scrub of the layer strip bakes nothing: the shadow costs one
  textured quad a frame. A drag of the gizmo bakes every frame, one more vertex pass over
  the moving plate into a small target; that is the frame to time on a large model.
- The shadow is a contact shadow, not the key light's: it does not move with the light, and
  a model lifted more than 30 mm off the plate casts none. A plate larger than about
  400 mm on a side gets a texel coarser than the blur.
- The cube has no depth and no light of its own and needs nothing from the renderer, so it
  draws the same in the browser build.
- Without the deck the plate's surface is a flat fill that takes no depth: a model below
  the plate shows through it, as it would under the x-ray. Every pipeline,
  vertex layout and shader entry the machine needed is deleted with it.
- Reopen the shadow if a print standing high on supports reads as floating, which is what
  a key-light shadow would answer.

## Alternatives considered

### A shadow map of the key light with PCF

The true shadow, falling on the models too, but one more depth pass over every triangle on
every frame the camera moves, or a cache keyed by the light as well as the scene, and a
filtered comparison sampler a WebGL2 build has to be checked for. The plate shadow is what
the design asks for.

### A second wgpu pass for the cube

Real light and depth on the cube, at the price of a second viewport, its pipelines and a
pick through the GPU. A flat cube in egui reads the same at 76 points.

### The option that won, and what it costs

A shadow that ignores the light's direction and stops at 30 mm, a texture of fixed
resolution whatever the plate, and a hash computed every frame; and a cube whose names do
not foreshorten with their faces.

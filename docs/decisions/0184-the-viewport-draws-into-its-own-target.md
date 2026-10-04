# 0184. The viewport draws into its own target

- **Status:** Accepted
- **Date:** 2026-10-04

## Context

The section cut is capped through a stencil count (ADR 0062, 0074), and the stencil plane
came from egui: the window asked eframe for `depth_buffer: 24, stencil_buffer: 8`, and the
scene was drawn straight into egui's pass (ADR 0014).

eframe's web painter passes a stencil width of zero to `depth_format_from_bits` whatever
`WebOptions` asks for, so in a browser the depth buffer has no stencil plane. The web build
therefore left the cut open, and the browser showed a different picture from the desk: a
section read as a shell with its inside surfaces washed, not as a part cut through.

## Decision

The viewport owns its colour plane and a `Depth24PlusStencil8` plane, the size of the
window, in `render::target`. In the callback's `prepare` the scene is drawn into them, in a
pass of its own recorded on egui's encoder, with the viewport set to the very pixels egui
will hand `paint`. In `paint` one triangle copies the colour plane into egui's pass,
texel for texel, with the premultiplied blend the scene was drawn with; the colour plane is
cleared transparent, so the result is what drawing into egui's pass gave.

Both front ends take this one path. egui is asked for no depth buffer at all.

## Consequences

The cap no longer depends on what egui allocates, so the browser caps the cut as the desk
does, and the three capping draws have no `Option` around them.

Every frame costs a full-window colour and depth-stencil allocation, kept and remade only
when the window changes size, and one textured triangle over the viewport. Beside a pass
over the plate's triangles that is noise.

The copy reads by framebuffer position, so the planes must stay the size of the window
egui draws into. A second viewport in another window, or multisampling, would need the
planes per viewport and a resolve; either is the signal to reopen this.

## Alternatives considered

### Patch eframe's web painter to ask for the stencil

A one-line change, but a fork of eframe in `[patch.crates-io]` to carry through every
upgrade, for a field upstream may or may not take.

### Shade the opening instead of capping it in the browser only

ADR 0062 already rejected it: a lattice paints through it, and the browser would show a
different, wrong picture from the desk.

### The option that won, and what it costs

A copy per frame and a window-sized pair of planes the GPU did not need at the desk, paid
so that one renderer runs unchanged in both places.

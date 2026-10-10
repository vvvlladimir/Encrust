# 0061. Scrub the stack from a vertical rail over the viewport that cuts the model

- **Status:** Accepted, the floating rail superseded by 0217
- **Date:** 2026-09-21

## Context

ADR 0025 gave the stage a second bottom panel, the layer transport: a 52 point strip
holding play, previous, next, a horizontal slider over the stack and a two-line readout,
shown only in Preview mode. It had two limits.

It only existed in one mode. Nothing in the Prepare mode could look inside a model — the
one thing a user wants when they are about to hollow it, place supports under a ledge, or
check that a wall came out as thick as the Hollow tool reported. The viewport drew every
model whole, from outside, in both modes.

And it read the stack without showing it. The slider moved the exposure mask in the
inspector; the 3D view beside it did not change at all, so a layer was a picture in a
panel rather than a height in the part.

The window already carries the pieces this needs. The viewport is one wgpu callback with a
globals uniform behind it, `Preview` already owns which layer is being shown, and the
cards over the viewport are `egui::Area`s anchored to its rectangle by their corner.

## Decision

The transport strip is gone. In its place is one floating card anchored to the middle of
the right edge of the viewport, `panels/section.rs`, drawn in both modes: the height the
cut is at, a step button, a vertical `egui::Slider`, the other step button, and one
action under them — play in Preview, "show the whole model" in Prepare.

The rail moves a cut, and the viewport draws the models only up to it. The cut is one
height in plate millimetres, in the globals uniform as `section = (height, on, 0, 0)`;
the model fragment shader discards anything above it. The faces the cut exposes are back
faces, which the shader already lights as if they faced the camera, and they are washed
towards the accent so that the opening reads as material rather than as a hole in the
surface. The plate and the grid are not cut.

What the cut follows depends on the mode, and only one of the two owns a height:

- **Preview** cuts at the layer it is showing, `Preview::layer_z`, always. That is what a
  layer preview is, and it keeps one number behind the mask in the inspector, the readout
  on the rail and the model in the viewport.
- **Prepare** cuts at `workspace::Section::height_mm`, which is `None` until the slider
  is moved and becomes `None` again at the top of the range or on the rail's own button.
  The range is the bottom and the top of everything visible on the plate, and the step
  buttons move by the print's layer height.

The rail is not drawn when there is nothing to scrub: an empty plate in Prepare, or a
Preview with no stack. The hint for the second case is already in the inspector's Layer
mask block, which is where a sentence fits.

This supersedes the layer transport of ADR 0025. The rest of 0025 — fixed panels in a
fixed order, and cards as areas anchored to the viewport — stands, and the rail is one
more of those cards.

## Consequences

The Prepare mode can see inside a model with no new geometry, no second mesh and no cost
that follows the triangle count: the cut is one comparison per fragment. Hollowing, infill
and the drain holes of step 8e are all inspectable with the tool that is already in hand.

The stage is one panel simpler and the viewport is 52 points taller in Preview.

The cut has no cap. A cross-section drawn by discarding fragments leaves the solid open at
the plane, so a cut model reads as an open shell washed in the section colour rather than
as a filled face. Capping it needs a second pass with a stencil, or the model's own
contour at that height fanned into a face; either is a change to the render pass rather
than to the shader, and neither is worth making until the open cut is what a user
complains about.

Two modes drive the same control, which is the thing to watch. If a third arrives, or if
the Prepare mode grows a reason to snap its cut to layers, `cut_height` is where that goes
and `Section` is where the state goes with it.

## Alternatives considered

### Keep the strip and make it vertical inside a right-hand panel

A `Panel::right` of its own, beside the inspector. It would not float over the model and
would need no overlay rectangle. It also takes its width from the viewport in both modes
whether or not there is anything to scrub, and it puts two panels of chrome down the same
edge, which is the shape ADR 0025 rejected `egui_dock` for.

### Cut by moving the near plane, or by an out-of-range camera clip

A clip plane is what a camera already has. Using the near plane would cut along the view
direction rather than along Z, so the cut would swing with the orbit; that is a different
feature and not the one a layer height asks for.

### One height as the only truth, with the preview layer derived from it

`Section` would own a height in both modes and `Preview` would be told which layer that
lands on. It is one number instead of two, but it moves the play button, the step and the
clamp at the end of the stack out of `Preview`, which owns the stack, and into a panel.
The layer index is also the honest unit in Preview: a stack is 1600 layers, not 80 mm of
continuous height.

### The option that won, and what it costs

Two owners of one cut. `cut_height` has to be asked rather than read, the Prepare height
is meaningless in Preview and stays behind while that mode is open, and a reader of
`Section` alone cannot tell where the viewport is cutting. The alternative was a panel
reaching into the stack, and this keeps the stack's index inside `Preview` where the job
that built it already reports.

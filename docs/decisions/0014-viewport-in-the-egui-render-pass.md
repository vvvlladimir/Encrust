# 0014. Paint the viewport into egui's own render pass

- **Status:** Accepted
- **Date:** 2026-09-17

## Context

Step 5a puts a 3D view of the build plate inside a docked egui panel. The models it draws
occlude each other and the plate grid, so the pass that draws them needs a depth buffer.

egui's render pass has a colour attachment and, by default, no depth attachment: egui
draws in submission order and never needs one. A custom `egui_wgpu::CallbackTrait` is
handed that pass, and a pipeline whose depth state or sample count disagrees with the pass
it is used in is rejected by wgpu at draw time.

There are two ways to get depth. Render the scene into a texture we own in `prepare` and
blit that texture into egui's pass in `paint`, or ask egui to attach a depth buffer to its
own pass and draw straight into it. eframe exposes the second as
`NativeOptions::depth_buffer`, which egui-wgpu turns into a window-sized depth texture
cleared to 1.0 at the start of every frame.

## Decision

The viewport draws directly into egui's render pass. `main.rs` sets
`NativeOptions::depth_buffer` to 32 bits and `multisampling` to zero; `render::gpu` builds
both pipelines with `Depth32Float`, one sample, `depth_compare: Less` and depth writes on.
The two sides of that agreement are named constants in `render::gpu`, and `main.rs` reads
them rather than repeating the numbers.

Everything mutable happens in `CallbackTrait::prepare`, which gets the callback resources
mutably: the camera uniform, the plate line buffer, the instance buffer and any mesh that
has not been uploaded yet. `paint` only replays what `prepare` recorded, because egui
lends the resources out immutably there.

## Consequences

- No offscreen texture, no blit, no second copy of the frame. One pass draws the whole
  window, and the 3D content is clipped to the panel by the viewport and scissor egui has
  already set.
- The depth buffer is the size of the window rather than of the panel, and it is cleared
  once per frame for the whole window. Nothing else writes depth — egui's own pipeline
  compares `Always` and does not write — so sharing it costs nothing today.
- egui draws the UI after the callback and without depth testing, so panels, menus and
  tooltips always appear over the viewport. That is what we want; it also means the
  viewport can never draw over the UI.
- If a second 3D panel appears, for example the layer preview in step 6, it shares this
  depth buffer. Both are cleared together at the start of the frame, and neither may
  assume depth survives across callbacks.
- Turning multisampling on becomes a two-sided change: `NativeOptions::multisampling` and
  `SAMPLE_COUNT` have to move together or every draw fails validation. The constants sit
  next to each other for that reason.

## Alternatives considered

### Render to an offscreen texture and blit it in

Full control: our own colour and depth attachments, our own sample count, our own clear
colour, and a texture we could reuse for picking or screenshots. Rejected for step 5a
because it doubles the per-frame work for a view that has no such requirement yet — an
extra render target the size of the panel, a resize path when the panel changes size, and
a full-screen quad pipeline to composite it. It is the natural upgrade if the viewport
ever needs its own sample count or a post-processing step.

### Sort back to front and skip depth entirely

Possible for the grid alone, and it is what egui itself does. Rejected because a triangle
mesh occludes itself: painter's order is undefined within one model, and the facets of a
single object would fight.

### Drawing into egui's pass, and what it costs

We inherit egui's pass configuration. Sample count, colour format and clear behaviour are
not ours to choose, and a change in how eframe configures its pass — an upgrade turning
multisampling on by default, say — breaks every pipeline here at once, at draw time rather
than at compile time. The depth buffer also costs window-sized memory whether the viewport
panel is open or closed. We accept that for a first viewport that needs none of the
control the alternative buys.

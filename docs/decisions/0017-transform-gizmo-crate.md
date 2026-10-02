# 0017. Use transform-gizmo-egui for the transform handles

- **Status:** Accepted
- **Date:** 2026-09-17

## Context

Step 5b needs handles the user can drag to move, rotate and scale the selected model. A
complete gizmo is more work than it looks: three translation axes and three plane handles,
three rotation rings plus a view-aligned one, uniform and per-axis scale, hit testing in
screen space with the right priority when handles overlap, a drag that holds the point
under the cursor rather than following it, and the whole set staying a constant size on
screen whatever the zoom.

`transform-gizmo-egui` does all of that and draws with ordinary egui shapes. The risk is
the one ADR 0002 already names for `egui_dock`: another crate that has to move in lockstep
with egui, on a project that had to rewrite its window code for egui 0.36 before it had a
single feature.

## Decision

`encrust-app` depends on `transform-gizmo-egui` 0.11, which builds against egui 0.36 — the
version this workspace already pins. `gizmo.rs` wraps it: it owns the `Gizmo`, converts
our `f32` `Transform` and matrices to the `f64` `mint` types the crate takes, and hands
back a `core_geometry::Transform`.

The wrapper is the whole surface. No other module names a type from the crate, so
replacing it means rewriting one file of about a hundred lines.

The gizmo draws with egui's painter after the wgpu callback, so it appears over the 3D
view without a depth test. Its handles are therefore never occluded by the model, which is
what a gizmo should do.

## Consequences

- All three modes work in one step, including plane handles and view-axis rotation, which
  a hand-rolled version would not have reached.
- The camera has to yield to the gizmo, or a drag on a handle would also orbit. The
  viewport reads `is_focused()` before it reads its own input, so the value is one frame
  old; that only matters on the frame the cursor crosses a handle.
- One more crate pinned to an egui version. An egui upgrade now needs `egui_dock` and
  `transform-gizmo-egui` to have released for it. Both are single-purpose crates with a
  history of tracking egui promptly, and the wrapper keeps the blast radius to one file.
- The gizmo works in `f64` and we work in `f32`, so every frame of a drag converts twice.
  That is a dozen values per frame and not worth avoiding, but it does mean a rotation
  comes back as a quaternion that has drifted off unit length and has to be renormalised.
- Our own interaction is not covered by tests. What `gizmo.rs` tests is the conversion in
  both directions, including that matrices are handed over row by row rather than by
  column — the mistake that would otherwise show up as handles that point the wrong way.

## Alternatives considered

### Hand-rolled handles

About two hundred lines for translation alone, and considerably more for rotation rings
and scale, with the hit-testing and constant-screen-size work repeated for each. Rejected
because it is a solved problem with no project-specific requirement, and because the
version risk it avoids is smaller than the maintenance it creates. It stays the fallback:
if the crate stops tracking egui, translation alone is a weekend and covers most of what
placing a model on a plate needs.

### egui-gizmo

The predecessor of this crate, by the same author. Rejected because it is superseded and
no longer tracks recent egui releases.

### transform-gizmo-egui, and what it costs

This is the third crate whose release schedule now gates our egui upgrades, and the one
with the least project-specific value if it stalls — the viewport and the dock layout are
load-bearing, a gizmo is replaceable. It also brings `mint`, `enumset`, `ecolor` and a
second copy of `glam` at a different minor version into the tree, which is real compile
time for a feature that draws a few hundred triangles. We took it because reimplementing a
correct gizmo is a step of its own, and this step already has picking in it.

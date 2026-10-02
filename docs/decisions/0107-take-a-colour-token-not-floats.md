# 0107. Take a colour token, not floats, at every renderer boundary

- **Status:** Accepted
- **Date:** 2026-09-27

## Context

`ui::theme` was supposed to be the only place a colour is written (ADR 0024), but the
viewport never obeyed: thirteen colours lived as `[f32; 4]` constants in `render/callback`
and `render/grid`, two as `vec3<f32>` in `shader.wgsl`, and the selection green as a
`Color32` in `panels/viewport_panel`. Three of them were duplicates of a fourth, and
`render/offscreen` carried four more inline where no `grep` for a constant would find
them. A rule nothing enforces is not a rule.

The renderer cannot take `Color32` all the way down: the uniform and vertex buffers are
`bytemuck::Pod` and want plain arrays. So a conversion has to happen somewhere, and it is
not free of meaning. `Color32` is gamma-space; `Rgba::from` decodes to linear and
`to_normalized_gamma_f32` does not. `egui-wgpu` writes gamma when the target format is not
sRGB and linear when it is, and our pipeline shares egui's `target_format`, while
`shader.wgsl` writes its colour out with no branch — so today's values are gamma.

## Decision

A colour crosses into the renderer as a `Color32` token and is converted once, inside the
constructor that has to store floats: `ModelInstance::new`, `LineVertex::new` and
`ExposureBand::new` take a `Color32`, and their colour fields are private. The one
conversion is `theme::gamma`, which is `to_normalized_gamma_f32` and does not linearise.
The shader's two colours become fields of the `Globals` uniform. Viewport colours are a
`theme::Scene` beside the `Palette`, because printed-resin grey is not window chrome and
the palette's luminance ladder does not apply to it.

`clippy.toml` bans every `Color32` constructor through `disallowed-methods`, and
`ui/theme.rs` carries the single `#![expect]` that licenses them. CI already runs
`cargo clippy --workspace --all-targets -- -D warnings`, so no new job is needed, and the
rule fires in the editor rather than only on a push.

## Consequences

A colour cannot be spelled outside `ui/`: either clippy rejects it or it does not
type-check. The type change is what found the four inline copies in `render/offscreen`,
which no textual search had. `#![expect]` rather than `#![allow]` means a typo in a
`clippy.toml` path turns into an unfulfilled-expectation warning instead of a silently
dead gate.

Two gaps remain. A `bytemuck` struct field is still a bare array, so a colour could be
written straight into a new `Globals` field; there is one such struct and it is read only
from tokens. And the shader writes gamma to whatever egui hands it, which is wrong on an
sRGB target — a pre-existing fault this decision documents rather than fixes, and the
signal to reopen it is a washed-out viewport on a platform whose surface is sRGB.

## Alternatives considered

### A grep script in CI

Would have caught the WGSL constants too. It lost because it does not run locally or in an
editor, it lives outside `cargo`, and the honest pattern for a colour literal cannot be
told apart from `LIGHT_DIRECTION` or a vertex position without an allowlist.

### `Rgba::from`, converting to linear

Physically correct for the lighting the shader then does. It lost because it changes every
colour in the viewport, which is a look decision and not a tokens decision.

### The option that won, and what it costs

Threading `Color32` through the constructors puts an `egui` type into `render/vertex`,
which now depends on `ui::theme` — the renderer and the design system are coupled where
they were not. That is acceptable only because both live in `encrust-app`, which already
depends on `egui`; it would be wrong in a `core-*` crate. It also spends a per-vertex
`u8`-to-`f32` divide that a precomputed array did not, on a buffer that is rebuilt every
frame.

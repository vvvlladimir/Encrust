# 0215. The panels are pictured and compared, by hand

- **Status:** Accepted
- **Date:** 2026-10-10

## Context

The window is about to be redrawn panel by panel: new tokens, a new face, new widgets, then a
new frame around them. Each of those steps is meant to change how the window looks and
nothing it does, and a test of behaviour cannot see a field that moved, a label cut off or a
fold that no longer opens. Until now the only check of the window's look was a person going
through it.

egui draws the window itself, so a picture of it can be taken without a screen:
`egui_kittest` runs frames against a `Harness` and renders them through `egui-wgpu`, which the
window already uses. Rendering needs a GPU adapter, and the Linux runner in `ci.yml` has
none; that job compiles the window and tests only what opens no window.

## Decision

`encrust-app` takes `egui_kittest` 0.36 as a dev-dependency, with its `snapshot` and `wgpu`
features. `app::snapshots` draws the whole window — `Window::show` on a `SlicerApp` with the
Saturn 4 Ultra applied and, where a tool needs one, a 20 mm cube picked — and compares it
with a PNG in `crates/encrust-app/tests/snapshots/`. One picture per state: the empty plate,
a picked model, every tool, Preview, the Settings screen and the sheet of keys.

The viewport is in every picture but draws nothing: its GPU resources are installed by
`SlicerApp::new`, which the pictures do not call. They pin the chrome, not the scene.

The module compiles only under the `snapshots` feature, which nothing turns on by default, so
CI never runs it. Whoever changes how the window looks runs

```sh
cargo test -p encrust-app --features snapshots --lib snapshots
```

before and after, and `UPDATE_SNAPSHOTS=1` writes the new pictures to be committed with the
change. The `.new`, `.diff` and `.old` files a run leaves are ignored by git.

## Consequences

A redraw comes with the pictures it changed, so a review sees what moved, and a change meant
to leave the look alone shows when it did not. The dev-dependency builds nothing into a
release and `about.toml` ignores dev-dependencies, so the shipped terms are unchanged.

Nothing enforces the pictures: a change that forgets to run them passes CI with stale PNGs.
The pictures were taken on macOS, and another platform's renderer may differ by a few pixels
inside `egui_kittest`'s default threshold, or past it. Twenty pictures at 1280 × 800 are about
1 MB in git, and each redraw adds its own.

Revisit if a runner with a GPU, or a software adapter that matches macOS closely enough,
becomes available: then the feature goes into CI as it is.

## Alternatives considered

### Running them in CI on a software adapter

Mesa's lavapipe gives the Linux runner a Vulkan adapter. Rejected for now: its rasterisation
differs from Metal's, so the committed pictures would either be Linux's, which nobody here
sees, or carry a threshold loose enough to miss what they are for.

### Layout checks without pictures

`egui_kittest` without `wgpu` runs in CI and can find a widget by its label and read its
rectangle. Rejected as the only check: a redraw changes colours, radii and type, and none of
that is in a rectangle.

### A person and a list, and what this costs

The parity list and a run through the window still happen for each step; the pictures catch
what a person skims past. The cost is a dev-dependency, a feature flag, and pictures that are
only as current as the last person who ran them.

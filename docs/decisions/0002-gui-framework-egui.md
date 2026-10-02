# 0002. Use egui and eframe for the desktop GUI

- **Status:** Accepted
- **Date:** 2026-09-17

## Context

The GUI needs a 3D viewport with an orbit camera, a transform gizmo, picking by raycast, a
layer preview scrubbing through hundreds of masks, and a dockable panel layout. It must
run natively and fully offline on Linux, macOS and Windows, and it must not fight the
background slicing thread.

The viewport is the constraint that matters. It is not a widget the toolkit provides; it
is our own wgpu rendering, and the toolkit has to let us paint into a region with our own
GPU pipeline and read input back out. Everything else is ordinary forms and lists.

## Decision

The GUI is built on `egui` with the `eframe` application shell, and `egui_dock` for the
panel layout. `encrust-app` produces the `encrust` binary; it is the only crate besides
`encrust-cli` allowed to depend on graphics crates.

egui's immediate-mode model matches how this application's state actually behaves. The
scene, the current layer index and the slicing progress are plain data that change from
outside the UI; rendering them means reading them each frame rather than keeping widget
objects in sync with them. There is no observer wiring to get wrong when a background
thread finishes a slice.

eframe renders through wgpu, which is the same API the viewport needs. Custom 3D painting
is `egui_wgpu::CallbackTrait` sharing the frame's device and queue — not a second graphics
context to manage, and not a texture copied between two stacks every frame.

## Consequences

- The viewport in step 5 is wgpu code inside an egui paint callback, with no bridging
  layer between the UI and the renderer.
- No system dependencies to install and no runtime beyond the binary. The project stays
  genuinely offline and trivially cross-compiled.
- State stays as plain Rust data owned by the app struct. A background slicing thread can
  publish progress through a channel and the UI picks it up on the next frame.
- The cost is visual polish. egui looks like egui, and it is not a native toolkit. Text
  layout, accessibility and complex text input are weaker than in a retained-mode toolkit.
- egui's API moves between releases — 0.36 renamed `App::update` to `App::ui` and reworked
  panels and menus. Upgrades are real work, and `egui_dock` has to move in lockstep.
- Very long scrollable lists re-evaluate every frame. If the layer preview or a profile
  library gets slow, the fix is virtualised rendering, not a different toolkit.

## Alternatives considered

### iced

Elm-style architecture, genuinely nice for forms, and it renders through wgpu. Rejected
because the message-and-update model fights a 3D editor: a gizmo drag or a camera orbit is
continuous mutable state that becomes a stream of messages and a growing enum, and custom
wgpu integration is less direct than egui's paint callbacks.

### Slint

The best-looking option, with a real design language and a declarative markup. Rejected
because the model is a separate `.slint` language with generated bindings — an extra build
step and an extra language between us and the code — and embedding custom wgpu rendering
is not its primary path. Its licensing also needs deliberate attention for an open-source
project, which is friction we do not need.

### Tauri

A web front end would give the most UI flexibility and the largest pool of contributors
who can style it. Rejected outright: it means shipping a webview, a JavaScript build
chain and an IPC boundary that every mask and every mesh has to cross. For an offline
tool whose hot path is moving megabytes of pixels to the screen, that is the wrong shape.

### egui, the option that won, and what it costs

egui is the least capable of these at building a beautiful interface, and immediate mode
means re-deriving the entire UI every frame — fine at 60 Hz for our widget count, but it
puts a ceiling on how much UI we can afford. The API churn is a recurring tax: this project
already had to rewrite its window code for 0.36 before the first feature existed. We accept
that because the viewport is the hard part of this GUI, and egui makes the hard part easy
while making the easy part merely adequate.

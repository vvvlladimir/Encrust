# Architecture rules

1. **Cores know nothing about the UI.** No `core-*` crate touches `egui`, `eframe`, `wgpu`,
   `winit` or any graphics or windowing crate; only the two binaries and `encrust-web` do.
   Every core runs from a CLI, a test and a benchmark with no window.
2. **Traits, not enum switches** — once a second implementation or a crate boundary demands
   it, never before.
3. **Simplicity beats generality.** No plugins, injection or event bus: plain functions,
   plain structs, explicit dependencies.
4. **Data is separate from algorithms.** `Mesh`, `Layer`, `Contour`, `LayerMask` carry
   constructors, accessors and pure conversions, never the work.
5. **Crate boundaries are responsibility boundaries.** A type two crates need moves down
   into a shared crate; never duplicated, never a sideways edge between peers.
6. **Errors:** `thiserror` in libraries, `anyhow` in binaries only.
7. **Scale is a design constraint.** Geometry queries go through the `Bvh`, neighbourhoods
   and pairs through a grid or `FastMap`, never a loop over every face or voxel. Work
   proportional to triangles, layers or tiles runs on `rayon`. Peak memory does not grow with
   layer count; sparse stays sparse. A search uses bisection or a closed form, not a fixed
   step to a far bound; every retry loop has a stated bound.
8. **Optimise on evidence:** a kept `criterion` benchmark decides.
9. **The CLI keeps pace with the window.** Everything the window does, `encrust-cli` does
   through the same core call; the window holds only input and drawing. A feature lands in
   both in one change, its flag in `docs/cli.md`.

The dependency graph is `docs/architecture.md`: read it before moving a type or adding a
crate; an edge it does not draw is forbidden, and `cargo xtask arch` checks it.

- A sliced-file format is a `format-*` crate on `core-format` alone, plus its container's
  crate if the container is foreign (ADR 0148).
- A printer protocol is a `net-*` crate taking a finished file's path and depending on nothing
  here, not even another `net-*` (ADR 0136); what two share is an enum in `encrust-app` (0153).
- `core-pipeline` spans core layers only to write a cut stack into a sink, `core-engine` only
  to run a plate to that call and hold `.encrust`; neither takes what two callers happen to
  share (ADR 0127, 0174).

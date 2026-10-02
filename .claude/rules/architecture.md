# Architecture rules

1. **Cores know nothing about the UI.** No `core-*` crate may touch `egui`, `eframe`, `wgpu`,
   `winit` or any graphics or windowing crate; only the two binaries may. Each core works from
   a CLI, a test and a benchmark with no window open.
2. **Extend through traits, not enum switches** — but only once the second implementation
   arrives or a crate boundary demands it. No abstraction ahead of its use.
3. **Simplicity beats generality.** No plugin system, no injection, no event bus. Plain
   functions, plain structs, explicit dependencies.
4. **Data is separate from algorithms.** `Mesh`, `Layer`, `Contour`, `LayerMask` carry
   constructors, accessors and pure conversions, never the work.
5. **Crate boundaries are responsibility boundaries.** Two crates needing one type moves the
   type down into a shared crate. Never duplicate, never add a sideways dependency between
   peers.
6. **Errors:** `thiserror` with specific variants in libraries, `anyhow` in binaries only.
7. **Scale is a design constraint, not a later fix.** Cost follows the work, not the model:
   geometry queries through the `Bvh`, neighbourhood searches through a grid or a `FastMap`,
   never a loop over every face or voxel. Anything proportional to triangles, layers or tiles
   runs on `rayon`. Peak memory must not grow with the layer count — stream layers and free as
   you go. Sparse data stays sparse.
8. **Optimise on evidence.** Write for clarity, then let a `criterion` benchmark decide, and
   keep that benchmark.

The allowed dependency graph is in `docs/architecture.md`, with why each edge is there and the
ADR behind it. Anything it does not draw is forbidden; read it before moving a type or adding
a crate.

A new sliced-file format is a new `format-*` crate on `core-format` alone, plus its
container's crate if the container is someone else's (ADR 0148). A new printer protocol is a
new `net-*` crate taking the path of a finished file and depending on nothing here, not even
another `net-*` (ADR 0136); what two of them share is settled in `encrust-app` by an enum over
destinations (ADR 0153). `core-pipeline` spans core layers only to write a cut stack into a
file, never to hold what two callers happen to share (ADR 0127).

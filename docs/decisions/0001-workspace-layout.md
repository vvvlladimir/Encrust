# 0001. Split the workspace into nine crates along pipeline stages

- **Status:** Accepted
- **Date:** 2026-09-17

## Context

A slicer is a pipeline: import a mesh, place it, cut it into layers, rasterise the layers,
write a printer file. On top of that sit two front ends, a CLI and a desktop GUI, and the
GUI pulls in eframe, egui and eventually wgpu — a heavy graphics stack.

Two things had to be true from the first commit. The slicing pipeline must be runnable
with no window open, so it can be tested, benchmarked and driven from a CLI or a future
build server. And compiling the GUI must not be a prerequisite for working on geometry,
because the graphics stack dominates build time.

A single crate makes both impossible: every test run drags in eframe, and nothing stops
the geometry code from reaching for an `egui::Color32` when it is convenient.

## Decision

The workspace is split into nine crates, one per pipeline stage plus the two binaries:

```
core-geometry  core-mesh-io  core-slicer  core-raster  core-supports
printer-profiles  format-goo  encrust-cli  encrust-app
```

Dependencies flow strictly downward toward `core-geometry` and `printer-profiles`, which
are leaves. The three peers `core-mesh-io`, `core-slicer` and `core-supports` never
reference each other. No `core-*` crate may depend on a graphics or windowing crate; only
`encrust-cli` and `encrust-app` may depend on more than one layer. The full graph is drawn
in `AGENTS.md`, which is the enforceable version of this decision.

Shared dependency versions live in `[workspace.dependencies]`; member crates take them
with `.workspace = true`, so two crates can never disagree about a version of glam.

`format-goo` currently owns both the `SlicedFileWriter` trait and the `PrintJob` type,
even though a trait shared by several formats logically belongs lower. That is deliberate:
there is only one format today, and inventing `core-format` for a single implementation
would be abstraction ahead of need. When `.ctb` arrives in step 8, both types move down
into a new `core-format` crate and this ADR gets a successor.

## Consequences

- `cargo test -p core-geometry` compiles four small dependencies and finishes in under a
  second. Geometry work never waits on the graphics stack.
- The UI/core boundary is enforced by cargo, not by discipline. A `use egui` inside
  `core-slicer` is a compile error, not a review comment.
- Benchmarks and property tests see exactly the same API a caller sees.
- The cost is bookkeeping: nine `Cargo.toml` files, and moving a type between crates is a
  visible event rather than a quiet edit. That visibility is the point, but it does make
  early exploratory work slower.
- Rule 5 — a type needed by two crates moves down, never sideways — has to be applied
  consciously. The first time it is ignored the graph starts to rot.

## Alternatives considered

### One crate with modules

Fastest to start, and module privacy could express some of the same boundaries. Rejected
because module boundaries are not enforced against dependencies: nothing prevents the
slicing module from importing egui, and every test run would compile the entire graphics
stack. The build-time cost alone was decisive.

### Three crates: `core`, `cli`, `gui`

A reasonable middle ground, and it does keep the UI out of the core. Rejected because
`core` would become the grab bag the granular split is meant to prevent — geometry,
slicing, rasterisation and format encoding all able to reach into each other, with the
pipeline's actual shape invisible from the outside.

### Nine crates, the option that won, and what it costs

Nine crates for a project that has not sliced a single triangle yet is front-loaded
structure, and some of it is a guess. `core-supports` holds four types and no logic; if
support generation turns out to need deep access to slicing internals, the boundary
between it and `core-slicer` will have been drawn in the wrong place and will have to move.
The `format-goo` trait placement is a known temporary compromise. The bet is that
pipeline stages are stable boundaries in a slicer — every comparable project converges on
roughly these seams — and that the cost of moving a crate later is lower than the cost of
untangling a monolith.

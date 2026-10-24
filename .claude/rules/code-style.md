# Code style rules

## Comments and rustdoc

- None by default. One only for what code cannot say: a tolerance and why, a geometric edge
  case, a library bug with its link, a required order, a clause of an external spec.
- **Two lines at most.** Longer goes to `docs/`, the code keeps a pointer: `// see docs/x.md`.
- Never a restatement, a banner, commented-out code or a bare `TODO`; a TODO is
  `TODO(step-N): what is missing and where it goes`. Fix offending comments in code you touch.
- Every public item of a library: one or two lines of English rustdoc on what it is for, never
  how. Field docs only for a non-obvious unit or convention, the unit stated (mm, s, px).

## Errors

- Libraries: a `thiserror` enum whose variants carry what the caller acts on — path, index,
  value, never swallowed into an `Option`. Binaries: `anyhow`, `.context(...)` at each
  boundary a user recognises.
- No `unwrap`/`expect` in library code outside `#[cfg(test)]`; tests use
  `expect("why it cannot fail")`. A target that may panic (build script, integration test,
  bench) carries one file-level `#![expect(clippy::expect_used, reason = "...")]`.

## Naming, modules, size

- Names say what it does, with the unit when the type lacks it: `ScanlineRasterizer` not
  `RasterizerImpl`, `layer_height_mm`. Abbreviations: `aabb`, `bvh`, `rle`, `px`, `mm` only.
- A module is `foo.rs`, or `foo/` beside `foo.rs`; no `mod.rs`. `lib.rs` declares and
  re-exports, nothing else.
- A function aims at a screen, ~40 lines; past 80 code lines it fails. Split where a step has
  a name, never straight-line code into `part_1`/`part_2`; nesting past three levels is a helper.
- A file is one responsibility, ~400 code lines, failing past 600: then `foo/` with a file per
  part, like `core-supports/src/branch/`. A new concept is a new file, not an appendix.
- `cargo xtask hygiene` holds both by a ratchet; after a split, `--bless`. What is over a limit
  is split first, in its own commit, then extended. Dead code is deleted, never kept.

## Reuse

- Look before writing a helper: the `core-geometry/src/lib.rs` re-exports are the geometry
  catalogue (`Buckets::cell_of`, `inverse_matrix`, `cuboid`, …), then the crate itself.
- A second copy of a computation is extracted into the lowest crate owning the concept, so a
  tolerance has one home.

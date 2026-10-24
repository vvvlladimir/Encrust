# Code style rules

## Comments and rustdoc

- None by default. One only for what code cannot say: a tolerance and why, a geometric edge
  case, a library bug with its link, a required order, a clause of an external spec.
- **Two lines at most.** Longer goes to `docs/`; the code keeps a pointer
  (`// see docs/formats/goo.md`).
- Never a restatement, a banner, commented-out code or a bare `TODO`; a TODO is
  `TODO(step-N): what is missing and where it goes`. Fix offending comments in code you touch.
- Every public item of a library: one or two lines of rustdoc on what it is for, never how.
  Field docs only for a non-obvious unit or convention, the unit stated (mm, s, px).
- Code and docs are English.

## Errors

- Libraries: a `thiserror` enum whose variants carry what the caller acts on — path, index,
  value. Binaries: `anyhow`, `.context(...)` at each boundary a user recognises.
- No `unwrap`/`expect` in library code outside `#[cfg(test)]`; tests use
  `expect("why it cannot fail")`. A target that may panic (build script, integration test,
  bench) carries one file-level `#![expect(clippy::expect_used, reason = "...")]`.
- Never swallow an error into an `Option`.

## Naming, modules, size

- Names say what it does, with the unit when the type lacks it: `ScanlineRasterizer` not
  `RasterizerImpl`, `layer_height_mm`. Abbreviations: `aabb`, `bvh`, `rle`, `px`, `mm` only.
- A module is `foo.rs`, or `foo/` beside `foo.rs`; no `mod.rs`. `lib.rs` declares and
  re-exports, nothing else.
- A function ~40 lines: longer means a named helper, not denser code. A file ~400 lines of
  non-test code and one responsibility: past that, `foo/` with a file per part, like
  `core-supports/src/branch/`. What is over a limit is split first, in its own commit.
- Dead code is deleted, never kept behind a flag or a comment.

## Reuse

- Look before writing a helper: the `core-geometry/src/lib.rs` re-exports are the geometry
  catalogue (`Buckets::cell_of`, `inverse_matrix`, `cuboid`, …), then the crate itself.
- A second copy of a computation is extracted into the lowest crate owning the concept, so a
  tolerance has one home.

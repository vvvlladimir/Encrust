# Code style rules

## Comments

Default is none. Write one only where the code cannot carry the fact itself: a tolerance or
epsilon and why that value, a geometric edge case, a library bug with a link, a required
operation order, a clause of an external format specification.

- One line wherever one line will do. **Two lines is the hard limit.** Longer material goes to
  `docs/` and the code keeps a pointer: `// Traversal order is fixed by the specification, see
  docs/formats/goo.md`
- Never a restatement of the code, a section banner, commented-out code, or a bare `TODO`. A
  TODO reads `TODO(step-N): what is missing and where it goes`.
- Delete comments breaking these rules in the code you are already touching.

## Rustdoc

One or two lines on every public item of a library crate: what it is and what it is for, never
how it is implemented. Field docs only where a unit or convention is not obvious from the
name; state units explicitly (mm, seconds, pixels). Code, rustdoc, `docs/`, `.claude/rules/`
and `CLAUDE.md` are English.

## Errors

- Library crates define a `thiserror` enum whose variants carry what the caller needs to act:
  the path, the index, the offending value.
- Binaries use `anyhow` and add `.context(...)` at every boundary a user would recognise.
- No `unwrap()` or `expect()` in library code outside `#[cfg(test)]`; in tests prefer
  `expect("why this cannot fail")`. `clippy::unwrap_used` and `clippy::expect_used` enforce it,
  so a target that may panic — a build script, an integration test, a benchmark — carries one
  file-level `#![expect(clippy::expect_used, reason = "...")]`.
- Never swallow an error into an `Option`.

## Naming, modules and size

- Units in the name when the type lacks them: `layer_height_mm`, `exposure_s`, `width_px`.
- Implementations named for what they do: `ScanlineRasterizer`, not `RasterizerImpl`. Only
  established abbreviations: `aabb`, `bvh`, `rle`, `px`, `mm`.
- No `mod.rs` grab bags: a module is `foo.rs`, or `foo/` with `foo.rs` beside it. `lib.rs`
  declares modules and re-exports; it holds no logic.
- Functions fit on a screen, roughly 40 lines. Longer means a named helper, not denser code.
  Delete dead code rather than keeping it behind a flag or a comment.

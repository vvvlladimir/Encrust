# 0004. thiserror in libraries, anyhow in binaries, no unwrap in library code

- **Status:** Accepted
- **Date:** 2026-09-17

## Context

Almost everything in this pipeline can fail on input the user chose: a malformed STL, a
non-manifold mesh, a model taller than the build volume, a profile with a zero-sized
display, a layer whose contours do not close. Some of those failures the GUI must react
to specifically — highlight the offending triangles, offer to auto-repair, disable the
slice button. Others are terminal and only need to be explained to the user.

Two consumers need different things from the same errors. `encrust-cli` wants a readable
chain printed to a terminal. `encrust-app` wants to match on what went wrong and put a
control next to it. An error type that only carries a string serves the first and fails
the second.

## Decision

Library crates define their own error enum with `thiserror`. Variants are specific and
carry the context needed to act on them: the path, the layer index, the offending value,
the expected value. The source error is attached with `#[source]` or `#[from]` so the
chain survives.

Binaries use `anyhow`. Every boundary the user would recognise adds `.context(...)`:
"cannot load model.stl", not a bare io error.

`unwrap()` and `expect()` are forbidden in library code outside `#[cfg(test)]`. Inside
tests `expect("why this cannot fail")` is preferred, because the message is what a future
reader sees when the assumption breaks.

An error is never collapsed into `Option` when the caller might need to distinguish
causes. `Option` means "absent and that is fine" — `Mesh::aabb` on an empty mesh — not
"failed".

## Consequences

- The GUI can match `MeshIoError::Malformed { .. }` and offer repair, while matching
  `MeshIoError::Io { .. }` and offering a file picker, from the same call.
- Error text is written once, next to the variant, and cannot drift from the data it
  formats.
- Adding a variant is a breaking change for anyone matching exhaustively. Within this
  workspace that is a feature: the compiler points at every place that has to decide.
- The cost is verbosity. `MeshIoError` has four variants before a single format is fully
  parsed, and mapping io errors into them adds a closure at each call site.
- Tests assert on variants, never on message text, so wording stays free to improve. This
  is enforced in `AGENTS.md`.

## Alternatives considered

### anyhow everywhere

Far less code: one error type, `?` works across every boundary, context reads well. Used
by plenty of good Rust applications. Rejected because `anyhow::Error` is opaque —
recovering the cause means downcasting to a concrete type that has to exist anyway, or
matching on strings. The GUI is the whole reason this project exists, and it needs to
branch on failure.

### A single workspace-wide error enum

One `SlicerError` in a shared crate, with a variant per failure across all crates. Simpler
conversions and no `#[from]` chains between layers. Rejected because it inverts the
dependency graph: `core-geometry`, a leaf, would have to know about `.goo` encoding
failures. It also makes every crate's public API depend on every other crate's failures.

### Per-crate thiserror, the option that won, and what it costs

It is the most code of the three. Each crate carries an error module, each cross-crate
call needs a `#[from]` or a `map_err`, and a variant added in `core-geometry` can ripple
upward through three crates before it reaches the user. Error enums also tend to grow
variants that only one caller ever matches on, and pruning them is a breaking change
nobody prioritises. We accept that because the alternative is a GUI that can only say
"something went wrong".

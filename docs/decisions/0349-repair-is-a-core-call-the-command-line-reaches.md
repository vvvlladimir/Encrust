# 0349. Repair is a core call the command line reaches

- **Status:** Accepted
- **Date:** 2026-10-25

## Context

The repair the window runs, `mend` (ADR 0270, 0310), lived in `encrust-app/src/job/repair.rs`.
The Repair job and a tool's landing (ADR 0211) called it there, so the command line could
weld, drop doubled faces and orient a model but never close its holes. That breaks the rule
that the CLI reaches everything the window does through the same core call.

Every step of `mend` is already a `core-geometry` function: `collapse_flat_faces`,
`remove_duplicate_faces`, `orient_outward`, `fill_holes`, `diagnose`. Repair is also about to
grow methods (wrap, deep repair) that need the field and the exact engine, which live in
`core-volume`.

`core-volume` keeps a second chain of its own, `patched`, which readies a model for the
exact engine. It shares steps with `mend` but not its purpose: it cuts out faces around an
unpaired edge, and it never puts the model back, since the engine needs something it can read
even if it reads worse.

## Decision

**`mend` and `Mended` live in `core-geometry`, in `mend.rs`.** The window's Repair job, a
tool's landing and the command line all call it. The job's `Repaired` carries a `Mended`
rather than copies of its fields.

**The command line asks for it with `--repair`**, a flag of every command that imports a
model (`slice`, `estimate`, `batch`, `inspect`). It runs after the import's own
doubled-face and orientation passes, so a repair kept as found still leaves the model as a
plain import would. It conflicts with `--no-validate`. The text report gains a `repair` line,
the JSON `repair` object `holes_closed`, `holes_left` and `kept_as_found`.

**`core-volume`'s `patched` stays as it is.** Folding it into `mend` changes what the exact
booleans read, and is its own change with its own tests.

## Consequences

- The window and the command line repair a model the same way, by one function.
- Methods that need the field go into `core-volume` on top of `mend`, with no new edge in
  the crate graph.
- Writing a repaired mesh out (`encrust repair -o fixed.stl`) still needs a mesh writer,
  which `core-mesh-io` lacks; the flag repairs a model on its way to be sliced or inspected.
- Two chains of repair steps remain, one per purpose. The signal to revisit is a defect fixed
  in one that the other then needs.

## Alternatives considered

### Put the recipe in `core-volume`, beside `patched`

Close to where wrap will live. Rejected because nothing in `mend` needs `core-volume`, and a
leaf crate is reachable from every crate that might want it.

### A new `core-repair` crate

A clear name. Rejected as a crate for one module; it can be split out if repair outgrows
`core-geometry`'s scope.

### The option that won, and what it costs

`core-geometry` grows a recipe on top of its primitives, which it had not held before; a
recipe that needs a heavier crate has to live elsewhere, so repair is spread over two crates
once wrap lands.

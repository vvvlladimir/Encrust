# 0127. Orchestration is a crate, not a binary

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

`AGENTS.md` said only the two binaries may span layers. That is right
about graphics and windowing, and it was read as covering orchestration too, so the
sequence from a cut stack to a printable file was written twice: once in
`encrust-cli/src/{pipeline,sliced_file,stack}.rs` and once in
`encrust-app/src/job/{pipeline,format}.rs`.

The two copies had drifted into six duplicates — the `SlicedFormat` enum and its
extension lookup, `raster_settings`, the `PrintJob` assembly with its layer-height
exposure warning, the `match` that picks a writer, the window-at-a-time streaming loop,
and the fold that measures what cures. A fix to one was a silent bug in the other, and
the window's output could differ from the command line's for the same model.

## Decision

A new `core-pipeline` crate owns that stage: `SlicedFormat`, `raster_settings` over a
`PanelOverrides`, `fold_group`, and `write` — open the file, pick the writer, stream the
stack, close it. `measure` is the same loop writing nothing, for Preview.

What a front end still wants to hear goes through an `Observer` trait with three
defaulted methods — a window arriving, layers landing, and whether to stop. The CLI
implements it to count windows into its slicing report; the window implements it to push
progress down its channel and read its cancel flag.

The crate sits above every other core and below both binaries. It does **not** depend on
`core-plate`, `core-volume` or `core-supports`: which model to orient, hollow or support
is the front end's question, and only the write stage was ever duplicated.

## Consequences

The rule becomes "only the binaries may depend on graphics crates"; spanning core layers
is what `core-pipeline` is for. One path now writes every sliced file, so a cancelled run
deletes its half-written file in both front ends — previously only the window did.

`core-pipeline` is the widest core crate in the graph, which is what an orchestrator is;
it must stay the *write* stage and not grow into a place to put anything shared.

The CLI's `RasterReport` no longer folds layers itself: it is built from what `write`
returns, or counted into by the PNG path, which is the one caller left with a loop of its
own. That loop is over `fold_group`, so the rasterise-and-measure step is still shared.

## Alternatives considered

### Leave the duplication and add a test that pins both

Cheap, and it catches drift after the fact rather than preventing it. Two implementations
of a streaming loop over a distance-field-sized stack is exactly where a bug hides from a
test that only checks the bytes of a 4-layer box.

### Put the shared code in `core-format`

`core-format` is the file-format boundary that `format-goo` and `format-ctb` implement.
Giving it a dependency on the slicer's `Windows` and the analysis fold would make the
format crates transitively depend on the whole pipeline.

### `core-pipeline` owns orient, hollow and supports too

The audit's first sketch. Rejected: those stages are not duplicated — the CLI drives them
from flags and the window from tool state — so moving them would invent a shared shape
that neither caller wants, and pull three more crates into the graph for nothing.

### The option that won, and what it costs

One more crate to hold in the head, and an `Observer` trait with exactly two
implementations, which is the minimum that justifies a trait at all. If a third front end
never arrives, the trait stays a two-caller indirection — the price of having the two
callers share one loop instead of owning one each.

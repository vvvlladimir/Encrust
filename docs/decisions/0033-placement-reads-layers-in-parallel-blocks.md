# 0033. Read and examine layers in parallel blocks

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

With the area work moved onto the raster grid (`docs/decisions/0032`), a run of automatic
placement on an 80 mm ball spent about 125 ms of its time in two places: 20 ms reading
layers onto the grid, and 95 ms deciding what on each of them needs holding up. Both are
per-layer work on a machine with eight or more cores sitting idle, because the loop walked
the stack one layer at a time.

The loop looked sequential because it is written as one: a layer is read, compared against
the layers under it, sampled, and the samples are offered to the supports already standing.
Only the last of those four steps actually depends on what the layers before it did.
Reading a layer depends on nothing. Deciding what is unsupported depends on the layer and
on the fields of the layers below it, both of which are known before any support is placed.
Only the coverage check — "is this place already held by a support standing near and below
it" — carries state up the stack, and that state is what makes a run repeatable.

`core-slicer` already parallelises over layers with `rayon` for the same reason.

## Decision

`generate_supports` walks the stack in blocks of 32 layers. For each block it reads every
layer of it onto the grid in parallel, then works out in parallel what each of those layers
wants supports under, and then walks the block's answers in order through the one piece of
state a run has: the supports already standing.

`core-supports` therefore depends on `rayon`. The dependency graph in
`AGENTS.md` is updated to match.

A block carries with it the last `REFERENCE_RISE_MM / layer_height` fields of the block
before it, because that is how far back a lean is measured; the rest are dropped as soon as
the block is done with them, so peak memory follows a block and the window, not the stack.

`progress` is still called once per layer, in order, from the sequential walk, and a run
that is stopped keeps what it had placed up to that layer. A cancelled run therefore does
up to one block of work it throws away, which at 32 layers is a few milliseconds.

## Consequences

- A run is three times faster again on an eight-core machine: the 80 mm ball went from
  125 ms to 45 ms, and the benchmark's ball of 600 layers from 20 ms to 8.4 ms.
- The answer is unchanged, layer for layer. The parallel half of the work is a pure
  function of the fields, and the half that is not — which sample points become supports —
  is still walked in stack order.
- A block of fields is held at once. A field is spans rather than pixels, so 32 of them plus
  the twenty-layer window is a few hundred kilobytes on any real model.
- `generate_supports` is called from a worker thread that is already inside a `rayon` pool
  in the window (`crates/encrust-app/src/job/place.rs`). Nested `install` is what `rayon` is
  built for: the inner work runs on the same pool, and the thread budget of
  `docs/decisions/0019` still holds.
- The signal to reopen this: a block size that leaves cores idle on short stacks, or a
  cancellation that feels slow. Both are one constant.

## Alternatives considered

### Parallelise the whole stack at once

Read every layer, decide everything, then walk the answers. Simplest of all, and the
fastest, but it holds a field per layer of the stack at once and reads the whole model
before the progress bar moves at all — and a run cancelled at 5% would already have done
all of the work.

### Leave the run sequential and make the per-layer work cheaper still

There is more to win there — the detection is 60 µs a layer on the worst shape in the
benchmark — but it is winning fractions of what a second core wins outright, on work that
is independently per-layer by construction.

### Put the coverage check in the parallel part too

It is the only thing stopping the whole run from being one `par_iter`. It cannot go: the
question "is this place already held" is answered against the supports placed lower down,
and answering it out of order would place a different set of supports every run. Repeatable
output is worth more than the milliseconds.

### The option that won, and what it costs

The block boundary is visible in two places. A cancelled run wastes up to a block, and a
stack shorter than a block gets no parallelism at all — a 2 mm part at 0.05 mm layers is 40
layers, so it is one and a bit blocks and the tail is serial. There is also now a
`rayon` dependency in a crate that had none, which is one more thing between placement and
a plain `cargo test`.

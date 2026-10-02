# 0067. Write the resin volume into the header at `finish`

- **Status:** Accepted
- **Date:** 2026-09-22

## Context

ADR 0066 made the CLI cut the stack a window at a time and hold none of it, and paid for
that by slicing twice: both sliced formats state the resin volume in their header, the
volume is the area of every contour of the stack, and the header goes out before the first
layer. So one pass counted and a second pass wrote.

That is a whole slicing pass spent on one `f32`. Both writers already have a seekable sink
— `.ctb` lays its header down again at `finish` to fill in the layer table — so the field
can simply be written when it is known.

## Decision

`LayerSink::finish` takes the volume the stack came to. `.ctb` sets it on the job it
already rewrites the header from; `.goo`, whose header is a fixed layout of a fixed
length, seeks back to zero and writes the header again over itself. `PrintJob::volume_mm3`
is what the header is *opened* with, and a caller that does not know it yet opens with
zero.

The CLI's write pass folds each window into the `SliceReport` as it goes and hands the
report's volume to `finish`, so a run slices once. The report is printed after the write
rather than before it.

## Consequences

Writing a file costs one slicing pass again: on the 60 mm ball with a 1 mm honeycomb, the
run is back to roughly what it was before windowing, at a third of the memory.

Print time and resin weight are derived from the volume, so they come right for free —
both formats recompute them when the header is laid down again.

A writer must now be seekable all the way to the end of the run, which both already were.
A future format that can only stream forward would have to buffer its header or state a
volume of zero; the `SlicedFileWriter` trait has said its sink is seekable since ADR 0045,
so that is not a new constraint, but it is now load-bearing rather than convenient.

The order of the CLI's output changed: the slice report follows the file rather than
preceding it.

## Alternatives considered

### Keep slicing twice

No format change, no seeking. Rejected: a quarter of the wall clock of a dense run, paid
on every write, to learn one number.

### `set_volume` on the sink before `finish`

Less disruptive to the signature. Rejected because it can be forgotten silently, where an
argument cannot.

### Drop `volume_mm3` from `PrintJob`

The volume would then have exactly one home. Rejected for now: the app knows it before it
opens the file, and the header's other totals are derived at `begin` for the writers'
own tests.

### The decision above, and what it costs

The header is written twice and the second write must produce exactly the same length as
the first, which is true of both formats today and is not checked anywhere. A field that
became variable-length — a comment, a name — would corrupt the file rather than fail, and
only a round-trip test would catch it.

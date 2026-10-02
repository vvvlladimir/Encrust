# 0010. Rasterise the stack in windows, never holding it in memory

- **Status:** Accepted
- **Date:** 2026-09-17

## Context

`docs/architecture.md` drew the pipeline as `Sliced -> Vec<LayerMask> -> .goo`, and
`format-goo::PrintJob` still holds a `Vec<LayerMask>`. At the size of a real machine that
collection cannot exist.

One mask for a Mars 4 Ultra is 8520 × 4320 bytes, 36.8 MB. A 165 mm model at 0.05 mm
layers is 3300 of them: 121 GB. The number is not near a limit, it is three orders of
magnitude past one, so no amount of tuning rescues the shape.

Rasterisation is embarrassingly parallel across layers, and the masks are consumed in
layer order by whatever writes them out. Those two facts together decide the shape: the
only question was how the parallel producer and the ordered consumer meet.

## Decision

`encrust_cli::stack::write_stack` takes the layers `window` at a time. A window is
rasterised **and PNG-compressed** in parallel with rayon, then its results are written to
disk in order, and only then is the next window started. `--raster-window` sets the size;
it defaults to `rayon::current_num_threads()`.

Peak memory is therefore `window × mask size` plus the compressed buffers, and is stated
in the flag's help rather than discovered on a long job.

Compression happens inside the parallel step, not in the writing loop, because deflate on
a 36.8 MB mask costs more than the rasterisation that produced it.

## Consequences

A stack of any height runs in bounded memory. Measured on a 10 mm cube at 0.1 mm on a full
Mars 4 Ultra panel: 100 layers in 0.92 s, peak resident 303 MB on an eight-core machine —
eight masks in flight, which is the model the flag promises. Extrapolated, a 3300-layer
job is about 30 s at the same footprint.

Layer order is preserved by construction: the writing loop is sequential and indexes from
the window's position, so there is no reordering to get wrong and no writer thread to
join.

The cost is a synchronisation point at every window boundary. Threads that finish early
idle until the slowest layer in the window is done and the whole window is written. On a
model whose layers differ wildly in contour count that wastes some of the machine; a
work-stealing pipeline would not.

`format-goo::PrintJob` still holds every mask and so still cannot be built for a real
model. It carries a `TODO(step-4)` naming this ADR; step 4 changes it to consume layers
rather than own them.

Reopen this if a profile of a long job shows the window boundaries costing real time, or
if a format needs random access to layers rather than a single ordered pass.

## Alternatives considered

### Hold the whole stack in a `Vec<LayerMask>`

What the architecture diagram said and what `PrintJob` was written for. It is the simplest
possible code and it works for the toy panels in the tests. Rejected on arithmetic: 121 GB
for one ordinary print.

### A producer-consumer channel

Rasterise on a rayon pool, push finished masks down a bounded channel, write them on a
dedicated thread. Better machine utilisation than windows, because nothing waits at a
boundary. Rejected because the bound on memory becomes the channel capacity plus whatever
is in flight, which is harder to state honestly, and because masks arrive out of order and
have to be re-sequenced by index before writing.

### The option that won, and what it costs

Windowing trades throughput for a memory bound that can be written on one line. The
boundary stall is real and is not measured anywhere: the benchmark in `core-raster` times
a single layer, so nothing in the suite would notice if the stall grew. The default window
is also tied to the core count rather than to available memory, so a machine with many
cores and little RAM gets a window it cannot afford, and only the flag saves it.

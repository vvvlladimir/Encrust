# 0020. A rasterised layer leaves core-raster as runs, not as pixels

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

`ScanlineRasterizer` produced a `LayerMask`: one byte per pixel of the whole panel. On an
Elegoo Mars 4 Ultra that is 8520 × 4320 = 36.8 MB per layer, allocated and zeroed for
every layer of a stack that is routinely a few thousand layers long.

Every consumer then walked those bytes again. `EncodedLayer::encode` run-length encoded
the mask from one end to the other, because that is what `.goo` stores; the PNG stack
deflated it. Neither wanted pixels — they wanted runs, or a stream they could compress.

Measured on the raster benchmark, a 30 mm disc on a full Mars 4 Ultra panel cost 12.4 ms
to rasterise and 1.2 ms to encode. Almost all of the 12.4 ms was the final pass: a running
sum over every pixel of the layer's bounding box, then `clamp`, `round` and a byte store.
Sub-scanline count made no difference to it at all — 16 samples and 4 samples both came
out at about 12.4 ms — which says the sweep was not the cost. The panel was.

That matters more as the project grows. The goal is every MSLA printer, and the panels are
getting larger, not smaller: a 16K panel is 4 times the pixels of the one measured here,
while the parts printed on it are the same size.

The way out is to never build the bitmap: a rasteriser can emit runs directly, and the
dark surround of a part is then a single run.

## Decision

`Rastered` carries a `LayerRuns`, not a `LayerMask`. `LayerRuns` is a layer expressed as
`Run { length: u32, value: u8 }` in reading order, with the panel size it covers. It is
built through `RunsBuilder`, which merges neighbouring runs of one value as they arrive
and pads whatever the caller did not write with dark pixels.

`Run` lives in `core-raster`, not in `format-goo`. It is the format-neutral currency of a
rasterised layer: the `.goo` and `.ctb` families encode runs directly, and a format that
wants a bitmap calls `LayerRuns::to_mask`. `LayerSink::encode` therefore takes a
`&LayerRuns`, and `format-goo` re-exports `core_raster::Run` rather than defining its own.

Inside the sweep, a row is no longer a dense array of coverage differences but a sparse
list of `(pixel index, delta)` pairs. A span contributes at most six pairs whatever its
length. Emitting a row sorts that list, walks it, and turns each stretch between two
differences into one run — so a solid interior of any width costs one run and no per-pixel
work at all.

`LayerMask` stays, as the dense form: the PNG stack and, from step 6, the layer preview
want pixels. `LayerRuns::from_mask` converts the other way for pixels that arrive from
outside the rasteriser.

Pixel indices are `u32` throughout, so `RasterSettings::validate` now rejects a panel with
more pixels than a `u32` can address, as `RasterError::PanelTooLarge`.

## Consequences

- Rasterising the benchmark disc went from 12.4 ms to 2.22 ms per layer at 16 samples, and
  encoding it to `.goo` from 1.24 ms to 21.8 µs. A blank layer went from 0.5 ms to 46 ns:
  it is now a single run, produced without touching memory proportional to the panel.
- Peak memory no longer follows the panel. ADR 0010's window used to hold `window × 36.8`
  MB of masks; it now holds `window` run lists, which for a real part are tens of
  kilobytes each. The bound in ADR 0010 still holds, with a much smaller constant.
- Cost now follows the contours. The 16-sample and 4-sample cases have separated — 2.22 ms
  against 0.62 ms — which means the sweep is what is left to optimise. That is the signal
  to reconsider exact-area coverage, which would replace the sub-scanlines outright.
- A second sliced-file format costs less to add. It receives runs, which is what its
  encoder wants, and does not have to re-derive them from pixels.
- The PNG stack pays for `to_mask` that it did not pay before. It is a debug artefact, it
  deflates the mask immediately afterwards, and deflate costs far more than the expansion.
- Anything that wants to read a pixel has to expand the layer or walk the runs. Tests that
  asserted on `mask.pixels()[i]` now call `to_mask()` first, and that is the ergonomic
  price of the change.

## Alternatives considered

### Keep the mask and make the final pass faster

SIMD the running sum, the way `font-rs` does its `accumulate` step with SSE. It is 2 to 4
times on that loop, needs either nightly `std::simd` or per-architecture intrinsics, and
leaves the 36.8 MB allocation, the byte stores and the encoder's second walk exactly where
they were. It optimises a pass that this decision deletes.

### Keep the mask but track the dirty box for the encoder

Carry the bounding box into `Rastered` so the encoder can emit the untouched rows as one
run. Cheaper to implement, and it removes the encoder's walk. It does nothing about the
allocation or about writing the interior a byte at a time, which is the larger half.

### Put `Run` in a new `core-format` crate

Architecturally tidier: a type shared by the formats, owned by neither. Rejected by
architecture rule 2 — the second format does not exist yet, and `core-raster` produces
runs, so `core-raster` is where they belong until `SlicedFileWriter` and `PrintJob` move
down together.

### The option that won, and what it costs

Runs are a worse data structure than an array for anything that indexes. Reading pixel
`(x, y)` is a walk, not an offset, and the preview in step 6 will have to expand a layer
or decode runs on the GPU rather than upload a texture directly. Sparse row deltas also
sort a small list per row, which is asymptotically worse than the dense array's linear
scan and only wins because the list is short — a pathological layer with thousands of
spans per row would sort thousands of entries 16 times per row. No such layer exists in a
sliced mesh, where spans per row follow the cross-section's complexity, but it is a real
edge of the design rather than a strictly better one.

Summation order also changed: differences at one pixel are now summed after an unstable
sort rather than in the order they were added. The result differs from the old one only in
the last bits of an `f32`, well under the rounding to 8 bits that follows, but it is not
bit-identical.

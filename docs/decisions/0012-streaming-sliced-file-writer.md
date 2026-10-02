# 0012. Hand layers to the sliced-file writer one at a time

- **Status:** Accepted; the sink's bound is amended by 0045
- **Date:** 2026-09-17

## Context

Step 0 gave `format-goo` a `SlicedFileWriter` trait whose single method took a `PrintJob`
holding `Vec<LayerMask>` and wrote the whole thing. That cannot be implemented. A Mars 4
Ultra panel is 8520 x 4320 pixels, one byte per pixel, so one mask is 36 MB; a 165 mm
model at 0.05 mm is 3300 layers, which is 121 GB of masks. The `TODO(step-4)` on
`PrintJob` said as much.

Step 3 already solved the same problem for the PNG stack: rasterise a window of layers in
parallel, write the window in order, drop it. See ADR 0010. The sliced file has to reuse
that shape rather than fight it.

Two facts about `.goo` make streaming straightforward. The header is a fixed `0x2FB95`
bytes, so `offset of layer content` is a constant rather than something discovered while
writing. And the layer count is known from `Sliced` before the first mask is rasterised,
so the header's `total layers` can be written before any layer exists.

What is *not* known before the layers are written is the exposed volume, the print time
and the resin weight. The header carries all three.

## Decision

`SlicedFileWriter::begin` writes the header and returns a `LayerSink`. The sink takes
layers one at a time with `push` and is closed with `finish`, which writes the ending
string and fails if fewer layers arrived than the header promised.

The compression step is split out of `push` as `LayerSink::encode`, an associated function
producing a `Send` value that borrows nothing. `encrust-cli` therefore encodes a window of
layers with rayon and pushes the results in order, exactly as it does for PNGs.

`PrintJob` stops holding masks. It carries the printer profile, the material profile, the
`RasterSettings` the masks were made for, the layer count and the print volume. The
caller computes the volume from the contour areas, which it can do before rasterising;
print time and weight follow from the volume and the material.

The sink writes to `&mut dyn Write`. It never seeks.

## Consequences

Peak memory no longer depends on the model: it is one window of masks, the same bound
step 3 established.

The header's totals are computed, not measured. Volume comes from the signed contour
areas, which is exact for a closed mesh but ignores what anti-aliasing does at the edges.
Print time ignores acceleration and the machine's own overheads, so it reads a few per
cent short of the printer's estimate. Neither number affects what is printed; both are
shown in the printer's UI and by any reader of the file.

If a real print shows those estimates to be badly wrong, the fix is to accumulate the
figures while streaming and seek back over the three fields, which means `Write + Seek`
and a sink that can no longer be a pipe or a `Vec<u8>` in a test.

`LayerSink` and `SlicedFileWriter` are generic over an associated type, so neither is
object safe. With one format that costs nothing. A second format still moves both traits
down into a `core-format` crate, as `docs/architecture.md` already says.

## Alternatives considered

### Keep `write(&job)` and spool the masks to a temporary file

Preserves the trait exactly as step 0 wrote it. Rejected: it writes every layer twice, it
needs a temporary file the size of the output anyway, and it buys nothing, because the
layer count and the fixed header size already remove the reason to hold the stack.

### Take `Write + Seek` and patch the totals at the end

Gives an exact volume, weight and print time, measured from the masks the printer will
actually expose. Rejected for now: it forces the sink to be a real file, so the round-trip
test cannot write into a `Vec<u8>`, and the three fields it would fix are informational.

### The option that won, and what it costs

The caller now has to know the layer count and the volume before it can open a file. That
is fine for `encrust-cli`, which slices before it rasterises, but it rules out a writer fed
by a generator that does not know how long it is. It also means a job whose layer count is
wrong fails at `finish`, after a whole file has been written, rather than at `begin`.

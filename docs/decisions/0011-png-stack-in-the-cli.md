# 0011. The PNG stack is a CLI artefact, written with `png`

- **Status:** Accepted
- **Date:** 2026-09-17

## Context

Step 3's milestone is that the CLI turns an STL into something a person can look at: one
greyscale image per layer. That output is how rasterisation is checked by eye and by
number before any printer format exists.

It is not a printer format. No machine reads a directory of PNGs, and `SlicedFileWriter`
in `format-goo` already exists for the formats machines do read. So the question was where
the encoder lives, not whether it is a format implementation.

Rule 1 of `AGENTS.md` keeps the core crates usable from a test with no
window open; rule 2 says a trait arrives when the second implementation does, not before.
`core-raster` currently does no file I/O at all.

## Decision

PNG writing lives in `encrust_cli::stack`, next to the streaming loop that drives it. No
new crate, no method on `LayerMask`, and `core-raster` keeps its hands off the filesystem.

The encoder is the `png` crate, 8-bit greyscale, one file per layer named
`layer_NNNN.png` zero-padded to four digits so the stack sorts by name.

`Compression::Fast` rather than the default `Balanced`: the stack is an intermediate
artefact, and the difference is most of the deflate time on a 36.8 MB mask.

## Consequences

The core stays I/O-free and the debug output cannot drift into being treated as a
deliverable, because nothing but the binary can produce it.

`png` is a narrow dependency: an encoder and a decoder for one format, which is also what
lets the integration tests read a written layer back and assert on its pixels rather than
on its file size.

The cost is that `encrust-app` will not get the PNG stack for free when it wants one. If
the GUI in step 5 wants to export a stack, this module moves — to a `format-png` crate or
into `format-goo`'s neighbourhood — and that move is the signal to reopen this.

Four digits is enough for 10 000 layers, which is 500 mm at 0.05 mm, past any machine this
targets. A taller stack still writes correctly but stops sorting by name.

## Alternatives considered

### A `format-png` crate

Symmetric with `format-goo` and immediately reusable from the GUI. Rejected by rule 2: one
implementation, one caller, and a crate whose whole content is thirty lines of encoder
setup. The symmetry is superficial — `format-goo` writes one file describing a whole job,
this writes a directory of pictures.

### A `LayerMask::write_png` method

The shortest path. Rejected by rule 4 of the architecture rules: `LayerMask` is data, and
this would put `std::fs` and an image codec into the crate that every other crate depends
on.

### `image` instead of `png`

One API for every format, useful if previews or thumbnails are wanted later. Rejected
because it pulls decoders for a dozen formats to write 8-bit greyscale, and step 4's
preview images are a `.goo` concern with their own encoding.

### The option that won, and what it costs

Keeping the encoder in the binary means it is only as testable as the binary is: the unit
tests in `stack.rs` reach it directly, but any other crate that wants a PNG has to
duplicate it or wait for the move. It also puts `png` and `rayon` in `encrust-cli`'s
dependency list, so the CLI now carries pipeline machinery and not only argument parsing
and console output — a widening of that crate's job that the crate table in
`docs/architecture.md` now has to state explicitly.

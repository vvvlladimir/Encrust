# 0148. Write the `.sl1` settings last, and refuse a stack it cannot carry

- **Status:** Accepted
- **Date:** 2026-09-30

## Context

Step 18c adds `.sl1` and `.sl1s`. It is the first format we write that is not a binary
container: a zip holding two `key = value` settings files, two colour previews and one
eight-bit greyscale PNG per layer. Two of its properties do not fit the shape the other
writers are built around.

The first is that an archive entry cannot be rewritten. Every other format leaves a hole in
a header and seeks back to it once the stack is cut — which is what ADR 0045 made the sink
seekable for, and ADR 0067 patches the resin volume through. `config.ini` states that volume
in `usedMaterial`, and a zip entry is closed the moment the next one opens.

The second is that the container has **no per-layer table at all**. One `expTime` and one
`layerHeight` hold for the whole stack. The only thing that varies is the bottom block, named
by `numFade` — a count the firmware fades over, which is exactly our transition layers.
Nothing carries a band of exposure by height, and nothing carries a layer of its own
thickness. `core_format::validate` only refuses those when the *profile* says the machine
cannot read them, so an SL1 profile with `per_layer_settings = true` would have them silently
flattened.

A third, smaller thing: the PNG encoding a layer needs is the one `encrust-cli` already had
for its debug stack of PNGs.

## Decision

**The two settings files are the last entries written**, behind the previews and the whole
stack. A reader looks entries up by name, so the order does not reach it, and nothing has to
be rewritten. The sink stays seekable because the trait requires it; this writer simply does
not seek.

**A job the container cannot carry is refused**, with a new `FormatError::FixedForWholeStack`
naming the format and the field: an exposure plan, or a stack that is not uniform. Writing it
flat would produce a file that looks right and prints wrong.

**`format-sl1` owns the greyscale PNG encoding**, and `encrust-cli`'s PNG stack calls
`format_sl1::encode_grey` rather than keeping its own copy. `png` and `zip` become
dependencies of a `format-*` crate for the first time; both were already in the workspace.

## Consequences

`.sl1` is the one format whose file is valid without a seek, which makes it the one that
could be streamed to a socket. Nothing does that yet.

A user who set an exposure band or an adaptive stack and then typed an `.sl1` name gets an
error instead of a file. That is louder than the warning the CLI already prints when the
printer's own format disagrees with the extension, and deliberately so: the warning is about
a file that will still print, and this is about one that will not.

`encrust-cli`'s directory of PNGs now depends on the Prusa format crate, which reads oddly
until you notice they are the same eight-bit greyscale image. `png` moves to the CLI's
dev-dependencies, where its tests still decode what it wrote.

Verified against an independent reader: it opens our `.sl1`, reports no issues, reads back 200
layers, `numFade` 8, `usedMaterial` 0.995 and the whole panel description; and its decode of
layer 100 matches our own mask **exactly**, over all 10 490 880 pixels, because an eight-bit
container quantises nothing. This is the first of the three step-18 formats to round-trip
grey with no loss at all — `.cbddlp` costs eight passes for nine steps (ADR 0146) and PW0
costs a nibble for sixteen (ADR 0147).

The signal to revisit the refusal is a machine that reads per-layer values out of
`material_notes` the way some readers write them. That is a reader's convention, not Prusa's,
and adopting it would mean writing values into a free-text field and hoping.

## Alternatives considered

### Buffer the whole archive and rewrite `config.ini` at the end

Keeps the entry order the vendor's own slicer uses. It loses on ADR 0010 and rule 7: peak memory must not
grow with the layer count, and a stack is hundreds of megabytes.

### Write `config.ini` first with a padded `usedMaterial`, then patch the bytes in place

Possible — a stored entry's payload is at a known offset — but it means writing a number in a
fixed width and seeking into the middle of someone else's container format. The order costs
nothing instead.

### Flatten a banded stack, and warn

What a slicer that wanted to be forgiving would do. It lost because the file it produces is
indistinguishable from a correct one: the bands are simply gone, and the first the user hears
of it is a part cured wrong.

### Keep the PNG encoding in `encrust-cli` and write a second copy in `format-sl1`

Two copies of one codec, which rule 5 forbids outright. The alternative that was actually
weighed was putting it in `core-raster` beside `LayerMask`, and that lost because it would
give a core crate a PNG dependency for the benefit of one format.

### The option that won, and what it costs

The entry order is ours rather than the vendor's. Every reader we know of looks entries up
by name — the zip central directory is an index, not a stream — but a reader that insisted on
`config.ini` first would reject our files, and the readers we test against would not say so.

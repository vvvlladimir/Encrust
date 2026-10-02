# 0167. Give the gcode zip its own crate, and move the PNG codec down into `core-format`

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

Step 21d adds the `.zip` eleven machines of the catalogue read: a plain zip holding one
`run.gcode` program, two previews and one eight-bit greyscale PNG per layer. Three things
about it had to be settled.

It is the **second** archive container. `format-sl1` already carried the greyscale and colour
PNG encoding — ADR 0148 put it there on the argument that one format needed it and a core
crate should not take a PNG dependency for one format's benefit. A second format needing the
same codec breaks that argument: rule 5 says two crates needing one thing moves it down, not
that the second crate depends on the first.

Its layer values are **per layer by construction**. `.sl1` states one exposure and one layer
height for the whole stack, which is why ADR 0148 refuses a banded or adaptive job under an
`.sl1` name. Here every number a layer uses is a line in that layer's own block, so the same
job is written exactly as it stands — the container is the most expressive one we write, not
the least.

Both archives also start `PK`. The sniffer that recognises a renamed file by its first twelve
bytes cannot tell them apart, and until now `PK` meant `.sl1`.

Finally, one source profile in the catalogue names `FILEFORMAT_ZIP` **and** a firmware class
of its own, for a Klipper-based board whose zip holds a different program under the same
extension.

## Decision

**`format-gcode-zip` is a new crate** on `core-format`, `core-raster` and `zip`, beside
`format-sl1` rather than inside it (ADR 0148's rule for a new container). Its public surface
is `GcodeZipWriter`, `GcodeZipReader` and `claims`.

**`core-format` owns the PNG codec**: `encode_grey`, `encode_colour`, `decode_grey` and
`png_shape`, with `png` as its dependency. Both archive crates and the CLI's debug stack of
PNGs call it, and no `format-*` crate depends on another.

**Nothing is refused for varying**: a banded exposure and a stack of mixed thicknesses are
written as the per-layer blocks they already are. `core_format::validate` still has the last
word on what the *machine* reads, through the profile's firmware flags.

**A `PK` file is told apart by its entries**, not its first bytes: `format_gcode_zip::claims`
answers whether an archive holds `run.gcode`, and the sniffer falls back to `.sl1` when it
does not.

**A source profile naming a firmware class waits.** The catalogue generator treats
`FILECLASS_<name>` as part of what names the container, so the Klipper variant is reported as
waiting on step 21g instead of being given a file of this shape.

## Consequences

The `.zip` is the only container we write that carries an adaptive stack and an exposure band
together, which makes it the one to reach for when testing either against a real machine.
It is also the only one whose motion is ours rather than the firmware's: the program says
when to lift, how fast and how far, so a mistake in those lines is a crash into the vat
rather than a wrong number in a header. That is why the lift and retract are covered by their
own tests, including the one that proves the plate ends at the layer's own Z.

`core-format` now depends on `png`. Every `format-*` crate already depended on it
transitively through `core-format`'s sibling, so nothing gained a dependency it did not have;
what changed is that a core crate carries an image codec, which is a cost paid once rather
than per format.

Two containers now answer to one extension and one magic. A zip of numbered PNGs with no
program at all — which a machine of this family also accepts, reading its own stored profile
instead — is refused by both readers. If somebody asks for it, the signal is clear and the
place to put it is this crate with the program optional, not a third one.

The step is **unverified against a machine**: no file written here has been printed, and no
independent reader has been run over one. What is checked is that our own reader reads back
every layer's Z and exposure, that the pixels survive the round trip exactly, and that the
program's lines match a reference program a machine of this family printed. The signal to
revisit any of the motion decisions is the first report from a real Shuffle or CGR.

## Alternatives considered

### Put the container in `format-sl1`, since both are zips of PNGs

They share the archive and the image codec and nothing else: one is two `key = value` files
with no per-layer anything, the other is a gcode program with nothing but per-layer values.
One crate would carry two unrelated writers for the sake of an eighty-line PNG module, which
is now shared from below anyway.

### Put the PNG codec in `core-raster`, beside `LayerMask`

Weighed and rejected in ADR 0148, and the reason still holds: `core-raster` is on the hot path
of every slice and has no business carrying a file codec. `core-format` is where "what every
sliced file shares" already lives.

### Sniff the two archives apart by their first entry

Cheaper than opening the central directory — the entry name sits at a known offset in the
first local header. It loses because the first entry is whatever the writer happened to put
there: our own two archives differ, and so do the two vendors'.

### Write the Klipper variant too, since the archive is the same

Its program is a different language — different image command, different light command — so
"the same archive" buys nothing. Writing our program for that board would produce a file it
opens and then does not print, which is the failure mode the generator exists to prevent.

### The option that won, and what it costs

A fifth `format-*` crate for a container that is eleven machines, and a core crate that now
carries a PNG codec. The alternative to the crate was a second writer inside `format-sl1`,
and the alternative to the move was `format-gcode-zip` depending on `format-sl1` — a sideways
edge between peers, which the architecture rules forbid outright.

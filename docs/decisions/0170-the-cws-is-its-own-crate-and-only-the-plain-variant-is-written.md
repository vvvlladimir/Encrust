# 0170. Write the `.cws` from its own crate, and only the plain variant

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

`.cws` is a zip holding a `slice.conf` of settings, one eight-bit greyscale PNG per layer and
a gcode program. That is close to the `.zip` of greyscale PNGs `format-gcode-zip` writes
(ADR 0167): same PNG codec, same shape of program, same per-layer freedom. What differs is
every name and every key — the settings are a separate entry rather than the program's header,
the keywords are `;<Slice>` and `;<Delay>` rather than `;LAYER_START`, and the moves are
relative where the other container's are absolute.

The extension has two further variants. One holds 24-bit images, the other a `manifest.xml`
and 32-bit images; both are another firmware's file with the same suffix, and eleven machines
in the source catalogue name them. A third container is now also a zip, so sniffing by first
bytes no longer tells the archives apart.

## Decision

`format-cws` is its own crate on `core-format`, `core-raster` and `zip`, as `format-sl1` and
`format-gcode-zip` are (ADR 0148). It writes and reads the plain variant alone: a machine
whose profile names `RGB.CWS` or `XML.CWS` is reported by the generator as waiting, with the
step that would bring it.

What the two gcode archives share is the PNG codec, which already lives in `core-format`;
nothing else is factored out, because a program's text is the container's own and a shared
"gcode writer" would be an abstraction over two vocabularies that agree on nothing.

A zip is told apart by what is in it: `format_gcode_zip::claims` asks for `run.gcode`,
`format_cws::claims` for `slice.conf`, and a `PK` file that answers neither is read as an
`.sl1`.

Our images are named `encrust0000.png` upwards, numbered from zero in at least four digits.
The stem may not end in a digit, or it would run into the number.

## Consequences

Five Nova3D machines can be sliced for, with per-layer exposure and per-layer thickness
written as they stand. A reader of ours takes a layer's exposure from the wait while the panel
is lit and its Z from the relative moves added up, so a file another slicer wrote reports that
slicer's numbers.

Three `claims` checks now stand between a `PK` file and a reader. Each opens the archive's
central directory and nothing more, and the order is fixed in `core-pipeline`.

The two other variants are dead weight in the generator's table until somebody asks for one of
their machines. That is the standing rule for the long tail: a codec is worth writing when a
machine needs it.

## Alternatives considered

### One crate for both gcode archives

They share the PNG codec and the idea of a program. They share no name, no keyword and no
convention — absolute against relative moves, one settings entry against none — so the crate
would be two writers under one roof with an enum over which vocabulary to use, and every
function taking that enum. Two crates with one codec below them is less code.

### Write all three variants now

The 24-bit variant is a different image encoding and the `manifest.xml` one a different
archive layout; neither is covered by the reader we transcribed from in enough detail to write
blind, and nobody has asked. They are named in the generator's table instead, which is where a
waiting machine belongs.

### The option that won, and what it costs

A crate per archive means the gcode-shaped code is written twice, once per vocabulary, and a
reader of each has its own line-by-line parse. The duplication is in the text, not in the
codecs, and the alternative was an abstraction over two formats that only look alike.

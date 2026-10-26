# 0361. Write the PWSZ archive in `format-anycubic`, its layers in PW0

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

The Photon Mono M7, M7 Pro, M7 Max, Mono 4 Ultra and the Photon P1 and P1 Max read a zip
archive, not the table container `format-anycubic` writes. Its settings call the layer codec
`pwszImg`, a vector codec of contour segments, yet the vendor's own slicer, read statically,
and an open-source writer both store every layer as `layer_N.pw0Img`: the four-bit runs the
table container already uses. Under ADR 0148 a sliced-file format is a `format-*` crate of
its own, but a separate crate would reach the PW0 codec only by an edge between two format
crates, which `docs/architecture.md` forbids, or by moving the codec down into a crate it is
not the concept of.

The scene entry records, per layer, how many separate pieces it cures and the largest's area.
`core-analysis` already groups a layer's runs into pieces.

The P1 has two panels side by side; a profile states one.

## Decision

The archive is a `pwsz` module of `format-anycubic`, beside the table container, writing its
layers with the same `EncodedLayer`. The crate takes `zip` and `serde_json`, both in the
workspace already, and `core-analysis`, whose `cure` counts a layer's pieces. A profile
states it as `output.pwsz` with one of five extensions — `pwsz`, `pm7`, `pm7m`, `pm4u`,
`pp1m` — and five machines have profiles. The P1 waits. The layout, the constants and their
sources are in `docs/formats/pwsz.md`.

## Consequences

Five more machines are in the catalogue, written as the vendor writes them where we know
how, and on a codec whose bytes are tested already. The archive is not read back: a reader
is a later step. `format-anycubic` now depends on the crate that analyses a layer, which no
other format crate does. A wrong guess shows on the machine as a file it refuses; the
signal to revisit is a file sliced for one of these machines read entry by entry, or a
machine that refuses ours.

## Alternatives considered

### A `format-pwsz` crate

Truer to ADR 0148's one format, one crate. It lost because the shared PW0 codec would have
to move down into `core-format`, which holds what every format shares and not one vendor's
codec, or into a new crate of its own for one codec and two users.

### The `pwszImg` vector codec

What the settings name. It lost because no writer we know of uses it, so no machine is
known to have printed one, and the sink is handed runs, not contours.

### The option that won, and what it costs

`format-anycubic` holds two containers and a zip dependency the table container does not
need. Several constants — `intelli_mode`, the firmware's motion constants, the M7 Pro's
template written for every machine but the P1 Max — are copied, not understood. Until one
of these machines prints a file from this writer, all five profiles carry a format that has
never reached firmware from here.

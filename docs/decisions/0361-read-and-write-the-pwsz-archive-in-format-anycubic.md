# 0361. Read and write the PWSZ archive in `format-anycubic`, its layers in PW0

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

The Photon Mono M7, M7 Pro, M7 Max, Mono 4 Ultra and the Photon P1 and P1 Max read a zip
archive, not the table container `format-anycubic` writes. Its settings call the layer codec
`pwszImg`, a vector codec of contour segments, yet the files these machines read
and an open-source writer both store every layer as `layer_N.pw0Img`: the four-bit runs the
table container already uses. Under ADR 0148 a sliced-file format is a `format-*` crate of
its own, but a separate crate would reach the PW0 codec only by an edge between two format
crates, which `docs/architecture.md` forbids, or by moving the codec down into a crate it is
not the concept of.

Two known files, for the M7 Pro and the Mono 4 Ultra, show what the machines read: every
entry deflated, the settings and the layer controller ahead of the layers, the resin named
`user_resin`, both peel stages paired as in the table container's `EXTRA`, the bottom
light-off apart, a blur of 2 with none, and every extent in `scene.slice` zero with each
layer one piece. `encrust info`, `convert` and the window must open a PWSZ like any other
sliced file.

The P1 has two panels side by side; a profile states one.

## Decision

The archive is a `pwsz` module of `format-anycubic`, beside the table container, writing its
layers with the same `EncodedLayer`; the crate takes `zip` and `serde_json`, both in the
workspace already. A profile states it as `output.pwsz` with one of five extensions —
`pwsz`, `pm7`, `pm7m`, `pm4u`, `pp1m` — and five machines have profiles. The P1 waits.

`PwszReader` opens the five extensions and, for a renamed file, an archive holding
`anycubic_photon_resins.pwsp`. The writer deflates every entry, writes what the job alone
decides before the layers and what sums them up after, and states the settings and
`scene.slice` as the known files do. The layout and constants are in `docs/formats/pwsz.md`.

## Consequences

Five more machines are in the catalogue, on a codec whose bytes are tested already. A PWSZ
opens everywhere a sliced file does, and its fuzz target guards the reader. Writing costs a
deflate pass per layer. Our archive differs from the known ones in entry order — three
summary entries follow the layers — and in the factory resins it does not list.

Reopen if a machine refuses an archive whose summaries follow its layers, or refuses ours
at all.

## Alternatives considered

### A `format-pwsz` crate

Truer to ADR 0148's one format, one crate. It lost because the shared PW0 codec would have
to move down into `core-format`, which holds what every format shares and not one maker's
codec, or into a new crate of its own for one codec and two users.

### The `pwszImg` vector codec

What the settings name. It lost because no writer we know of uses it, so no machine is
known to have printed one, and the sink is handed runs, not contours.

### Hold every layer and write the known files' order exactly

Peak memory would grow with the layer count, which `.claude/rules/architecture.md` forbids,
for an order no reader that looks entries up by name can see.

### Count each layer's pieces and extents with `core-analysis`

Truer than the known files' zeros, but it made `format-anycubic` the one format crate
depending on layer analysis, for numbers the machines do not need.

### The option that won, and what it costs

`format-anycubic` holds two containers and a zip dependency the table container does not
need. Several constants — `intelli_mode`, the firmware's motion constants, the M7 Pro's
machine settings written for every machine but the P1 Max — are copied, not understood.
Until one of these machines prints a file from this writer, all five profiles carry a
format that has never reached firmware from here.

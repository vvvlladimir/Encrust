# 0367. Read the PWSZ archive, and write it as the vendor slicer does

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

ADR 0361 wrote the PWSZ archive from the vendor writer read statically, with no file to hold
it against, and no reader: `encrust info`, `convert` and the window could not open a PWSZ,
ours or the vendor's. Two vendor files, for the M7 Pro and the Mono 4 Ultra, now show where
ours differs: every entry deflated (ours 1.5 MB against 44 kB), the settings and the layer
controller ahead of the layers, the resin named `user_resin`, both peel stages paired as in
the table container's `EXTRA`, the bottom light-off apart, a blur of 2 with none, and every
extent in `scene.slice` zero with each layer one piece.

## Decision

`format-anycubic` gains `PwszReader`, opened by the five extensions and, for a renamed file,
by its `anycubic_photon_resins.pwsp`. The writer deflates every entry, writes what the job
alone decides before the layers and what sums them up after, and states the settings and
`scene.slice` as the vendor's file does (`docs/formats/pwsz.md`). With the extents and piece
counts gone, `format-anycubic` no longer depends on `core-analysis`.

## Consequences

A PWSZ opens everywhere a sliced file does, and its fuzz target guards the reader. Writing
costs a deflate pass per layer. Our archive differs from the vendor's in entry order — three
summary entries follow the layers — and in the factory resins it does not list.

Reopen if a machine refuses an archive whose summaries follow its layers.

## Alternatives considered

### Hold every layer and write the vendor's order exactly

Peak memory would grow with the layer count, which `.claude/rules/architecture.md` forbids,
for an order no reader that looks entries up by name can see.

### The option that won, and what it costs

The extents we computed were truer than the vendor's zeros; a reader that wanted them has lost
them. This supersedes ADR 0361's dependency on `core-analysis` and its scene contents.

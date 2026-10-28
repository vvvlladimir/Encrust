# 0377. Write `.goo` revision 5.1 for the machines that read it

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

We wrote every `.goo` at revision 3.0. The vendor's slicer writes 5.1 for the Elegoo
Jupiter 2, chosen by the machine's name; its own files for that machine are 5.1. That
revision is not a few fields more: every field is little-endian, the layers sit behind
three tables with an MD5 at the end, each layer's mask is the panel's two halves, and the
runs are a different code at eight levels of grey. A Jupiter 2 handed a 3.0 file meets a
mask twice the width it decodes, in a code it does not read. Layout: `docs/formats/goo.md`.

## Decision

The revision is the profile's: `OutputFormat::Goo5`, `output = "goo5"`, beside `Ctb5` and
`Cxdlp4`, and the Jupiter 2's profile states it. `format-goo` writes it with `Goo5Writer`,
next to `GooWriter`, both through one header and one layer definition that take a
`GooVersion`; `GooReader` reads both, telling them apart by the version string.

Where the vendor's file and ours could differ, we follow the vendor except in one place:

- the first-stage retract is the resin's less the layer's thickness, as the vendor states
  it; a reader adds the thickness back, so a file reads as the resin it came from;
- a pause stops where the layer stands, as in 3.0 (`docs/formats/goo.md`), not at the
  200 mm the vendor writes with no pause set;
- a grey becomes its level by `>> 5`, as the vendor's writer does; anti-aliasing passes
  through at eight levels rather than 256.

## Consequences

A new machine on 5.1 is a line in its profile. The output name still picks only the
container: a hand-typed `.goo` is 3.0, and a profile that states `goo5` keeps it, as for
`.ctb` (ADR 0047).

The writer seeks back once a layer to fill in the mask table, and reads the finished file
once for the MD5 (ADR 0376). Whether the firmware checks the MD5, or would read a 3.0 file
after all, is not known; the vendor's own file is what we match.

## Alternatives considered

### Pick the revision from the machine's name in `format-goo`, as the vendor does

No profile change, but a list of names in a format crate that the catalogue has to keep in
step with, and a machine renamed in a profile silently changes revision.

### The option that won, and what it costs

A second `OutputFormat` for the same extension, carried through the pipeline, the window's
format picker and the command line, all of which list it.

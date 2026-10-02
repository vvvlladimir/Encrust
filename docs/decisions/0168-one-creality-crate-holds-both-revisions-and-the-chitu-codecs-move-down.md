# 0168. Write both `.cxdlp` revisions from one crate and move the Chitu codecs down

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

The Halot line reads one extension at two revisions that share nothing but their first
eleven bytes. Version 3 is big-endian and holds a layer as a list of vertical lines down
each column. Version 4 is the `.ctb` family's tables and codecs under Creality's magic:
little-endian, previews in run-length RGB15, layers in the seven-bit run-length form, and a
per-layer motion block in front of each. Both end with the same reflected CRC-32 over the
whole file.

Those two codecs already existed in `format-chitu`, which is a peer of any new crate: rule 5
of `.claude/rules/architecture.md` forbids a sideways dependency between them and forbids
duplicating the code instead. One extension at two revisions also has a precedent —
`SlicedFormat::Ctb(CtbVersion)`, where the output name says the container and the profile
says the revision through `at_revision_of` (ADR 0047).

## Decision

`format-creality` writes and reads both revisions, as `v3` and `v4` beside each other, with
the magic, the model code and the checksum shared between them.

The seven-bit run-length codec moves down into `core-format` as `Rle7Layer`/`decode_rle7`,
and the RGB15 preview record with it as `encode_rgb15`/`write_preview`. `format-chitu` keeps
its own `.cbddlp` eight-pass codec and takes both of these from `core-format`.

A machine states `output = "cxdlp3"` or `"cxdlp4"`; both write `.cxdlp`, and the revision
comes from the profile, as a `.ctb`'s does. A reader picks between the two on the revision
field behind the magic, whether the file was found by its extension or sniffed.

## Consequences

Two codecs now have one home, so a fix to either reaches the three containers that carry
them. `core-format` grows by what two families share and no more; anything only one of them
holds — the cipher, the `.cbddlp` passes, the vertical lines — stays in its own crate.

The fourteen Halot and CT machines in the catalogue can be sliced for. The area field of
both revisions carries everything a layer lights rather than its largest island, which is
what a writer handed runs can measure; if a machine turns out to read it for anything, that
has to be revisited, and the field is pointed out in `docs/formats/creality.md`.

A reader that wants a layer of version 3 has to walk every layer before it, because the
container has no table of offsets. That cost is eight bytes of reading a layer and is paid
once, at open.

## Alternatives considered

### A crate per revision

`format-cxdlp3` and `format-cxdlp4` would each be small and neither would carry a match on
a revision. But they would share the magic, the model code and the whole checksum routine,
and nothing else in the workspace wants those, so sharing them would mean a third crate
under both — three crates for one extension.

### Version 4 in `format-chitu`, where its tables already live

Tempting, because the layout is that family's. It loses on the name: a crate called
`format-chitu` writing Creality's container would put a machine's file in a crate named
after a different firmware, and the keyword in a profile would stop matching the crate that
serves it. The shared part is the codec, not the container, and a codec is what moved.

### The option that won, and what it costs

One crate with two revisions in it means a reader has to sniff the revision before it can
choose, so `core-pipeline` reads two bytes of the file even when the extension already
claimed it. It also means `format-creality` is the largest of the format crates. Both are
cheaper than a third crate holding the magic and the checksum for two others.

# 0147. Write every Anycubic extension from one crate, at version 1

- **Status:** Accepted
- **Date:** 2026-09-30

## Context

Step 18b adds `.pwmx`. The Anycubic containers are not shaped like the two formats already
written: instead of one header with offsets into fixed blocks, a file is a mark followed by
named tables, each stating its own length, and which tables exist depends on a version
number that runs 1, 515, 516, 517, 518. Twenty-three extensions share the container, and
which versions each supports differs — `.pwmx` takes 1, 515 or 516, while `.pm5s` takes 518
alone.

A reference `.pwmx` was read byte by byte, which settled what the
code alone did not: the preview counts its own table head into its stated length where the
other tables do not, a speed is millimetres a second rather than a minute, and a layer
table row carries its layer's own thickness rather than its height above the plate.

Two of its properties bear on the shape of the code. The layer codec is a single pass of
four-bit grey, so nothing about it depends on the version. And the container records no
mirroring at all: the reference file, converted from a `.sl1`, matches its source layer for
layer with no flip, which is ADR 0134's rule arriving from the other direction.

## Decision

One crate, `format-anycubic`, one writer, `AnycubicWriter`, and **version 1** only.
`AnycubicFlavour` names the seven extensions that version 1 covers with this codec, and it
changes nothing but the name of the file.

Only `.pwmx` appears in `SlicedFormat::CHOICES`. The other six are reached by typing the
extension, which is what decides the format anyway (ADR 0047): twelve entries on a picker
serves nobody, and a Photon Mono SE owner types `plate.pwms`.

The header's anti-alias level is written as **1**. The runs carry four bits of grey of their
own, and a level above one asks a machine to read that many one-bit passes it will not find.

The five-six-five packing of a preview pixel moves to `core-thumbnail` as `rgb565`, beside
the pixel it packs. Two format crates now need it and they differ only in the byte order of
the word, which each writes itself.

## Consequences

Seven extensions work from one codec, and the three tables version 1 has are all a machine
needs: there is no build volume, machine name or mirroring field to get wrong.

Grey costs four bits instead of eight. A pixel under 16 goes dark, so an edge whose coverage
is under 6.25 % is lost. That is far gentler than the eight-pass arrangement ADR 0146 had to
buy for `.cbddlp`, and the file is smaller for it: the same cube that comes to 134 MB as a
`.cbddlp` is 1.3 MB as a `.pwmx`.

The grey is verified rather than argued. Our reading of the codec decodes the reference file
to exactly what an independent reader decodes it to, over all 4 147 200 pixels of its first
layer; and that reader decodes layer 100 of our own cube to our mask quantised to those
sixteen steps,
with zero mismatches over 10 490 880 pixels and no issues reported in the file.

An independent reader recomputes a file's resin volume on load rather than reading the header, so its
report of that one field disagrees with the bytes for its own files as much as for ours. It
is not a field to verify against that tool.

The signal to add version 515 or later is a machine that needs it, which means an Anycubic
printer profile in the catalogue — there is none yet. The signal to add `.pws` is the same,
and it is a second codec rather than a second version.

## Alternatives considered

### A crate per extension, or a version enum spanning 1 to 518

Both invent structure for machines this project has no profile for. Version 1 is what every
extension we offer supports, and the later revisions differ by whole tables, which is a
writer's worth of work each to no one's benefit yet.

### All seven extensions on the format picker

The honest reading of "the front end offers these". It lost because five of the seven name
machines nobody has asked for, and the picker is the one place where length is a cost paid
by every user.

### Write the anti-alias level the reference file carries, 8

What the reference file carries, copied from the `.sl1` it was converted from. It lost because it
is not true of the file: there is one pass, and the field is the number a machine is told to
read.

### The option that won, and what it costs

Four bits of grey is the format's ceiling, not a choice, but writing only version 1 is a
choice, and it means a `.pwmx` we write cannot carry the second lift stage or the rest times
that 516 added. A user tuning those on an Anycubic machine gets a file that silently ignores
them — and because no Anycubic profile ships, nothing yet warns them.

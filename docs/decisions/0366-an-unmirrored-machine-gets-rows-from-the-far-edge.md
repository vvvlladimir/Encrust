# 0366. An unmirrored machine gets its rows from the far edge of the plate

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

ADR 0134 writes every mask as the plate stands, row 0 at its near edge. Seventeen files the
vendor slicer cut for one asymmetric part disagree in two ways. On every machine whose panel is
X-mirrored (`.ctb`, `.goo`, `.fdg`, `.zip`, Photon Workshop) the vendor's layer is ours turned
half round: the vendor lays rows from the far edge and bakes the X mirror in. The part comes out
the right way round, only in the opposite corner. On every unmirrored machine (`.ctb` Sonic Mega
8K S, `.phz`, `.cxdlp`) and in `.photons`, where the vendor does not bake the mirror, the
vendor's layer is ours flipped in Y: our part prints as its mirror image. `.cxline` matched as
it was.

## Decision

`core_pipeline::write_to` lays a file's rows from the far edge of the plate when the profile
has no `mirror_x`, and always for `.photons`; an X-mirrored machine and `.cxline` keep the plate
as it stands. The rule is `panel::file_panel`, applied only where a file is written: the preview,
the measuring and the PNG stack still see the plate as ADR 0134 lays it. `convert` takes the
file it reads as cut for the machine it converts for, and reverses each layer's rows where
the two containers lay that machine's plate from opposite edges.

## Consequences

A part on an unmirrored machine prints the right way round. An X-mirrored machine prints as
before, in the corner opposite the vendor's. `encrust info` shows a file of ours and the
vendor's alike for every unmirrored machine.

The FlashForge and Nova3D profiles with `mirror_y` and no `mirror_x` are flipped by this rule
too; no vendor file exists for them. Reopen on a print from one of them, or on any file the
vendor slicer writes that this rule does not reproduce.

## Alternatives considered

### Reverse the rows in `core-raster` for every format and bake `mirror_x` as the vendor does

What the vendor does, and it would move a part on a mirrored machine into the vendor's corner.
It changes the file of every machine that already prints correctly, for placement alone.

### The option that won, and what it costs

The row order of a file now depends on its machine and container, which a reader of our files
has to know to put the part back on the plate, and two X-mirrored layouts still disagree with
the vendor's by a half turn.

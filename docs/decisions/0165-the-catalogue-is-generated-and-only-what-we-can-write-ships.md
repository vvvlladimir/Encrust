# 0165. Generate the printer catalogue, and ship only machines we can write a file for

- **Status:** Accepted
- **Date:** 2026-10-01

## Context

The catalogue held five machines, each transcribed by hand. A picker with five machines in
it is a picker nobody finds their printer in, and there are roughly 150 MSLA machines with
published panel geometry — a public set of printer profiles carries one `.ini` per machine,
with the resolution, the illuminated area, the travel, the mirroring flags and a keyword
naming the container that machine's firmware reads.

Transcribing 150 files by hand is a transcription slip per file. Reading them at build
time would make the catalogue change under a rebuild and would need the network. And the
set covers twenty containers, of which this workspace writes five: `.goo`, `.ctb` from
version 4, the older `.cbddlp`/`.photon`, the Photon Workshop container at version 1 and
`.sl1`. Nothing here can write the other fifteen yet.

Two of the source's own files disagree with each other: the profiles and the machine list
published beside them differ on three machines.

## Decision

`cargo xtask gen-profiles --source <dir>` transcribes the source profiles into
`assets/profiles/printers`. It is run by hand against a local checkout, never from
`build.rs`.

- **Only machines whose container we write are emitted.** Every other machine is reported
  with the step that brings its codec, and no file is written for it.
- **The generator owns only what it generated.** A catalogue file carrying its marker line
  is rewritten; one without it was written by hand and is left alone, so a profile
  corrected against a real machine survives every later run. A generated file the source
  no longer names is reported, not deleted.
- **A `.ctb` below version 4 ships pointing at version 4**, which is the oldest revision
  we write. The file says so in its header, and the profile clears
  `firmware.per_layer_settings`, because a board of that generation predates the firmware
  that reads the per-layer tables.
- **Nothing is invented.** No exposure, no machine name, and `connection` only for the
  generations named in `docs/formats/sdcp.md` and `docs/formats/prusalink.md`. Every file
  carries an `UNVERIFIED` header.
- **The machine list is a cross-check, not a source.** A panel that disagrees with it
  fails the run. Where the list is the stale one, that judgement is a line in the
  generator naming what settles it.

## Consequences

The catalogue goes from five machines to 47 and grows by a flag rather than by typing.
Adding a container in a later step adds its machines by rerunning the generator. The three
things the source cannot give — a measured exposure, the machine name a firmware matches,
and how a machine is reached — stay the user's or a later reader's to supply.

A shipped resin can no longer be tuned for every shipped machine: 47 machines times four
resins is 188 exposures nobody has measured. A machine with no table resolves to the
numbers the resin was last measured with, and the window marks it as untuned.

Pointing a version-3 board at a version-4 file is the one claim here that can be wrong.
It fails safe — the firmware refuses a file it cannot parse, rather than misprinting it —
and a machine that refuses one is the signal to write version 3 after all.

## Alternatives considered

### Vendor the source `.ini` files and read them at build time

The catalogue would never drift from the source. But the build would depend on files we
did not write, a rebuild could change what ships, and a generated catalogue nobody reviews
is a catalogue nobody checked. Measurements are not ours to relicense either; transcribing
them into our own schema is.

### Ship every machine in the source, container or not

It would put 150 machines in the picker at once. A profile whose container has no writer
cannot produce a file, so picking one is a dead end with no explanation — worse than the
machine being absent.

### The option that won, and what it costs

Three sources of truth now have to agree: the source profiles, the machine list beside
them, and the generator's own table of judgements. Each judgement is a line somebody wrote
once and nobody will re-examine until a user reports that their machine is wrong. The
hand-written profiles are the other cost: five files in a different layout from the other
forty-two, which a reader has to be told about rather than see.

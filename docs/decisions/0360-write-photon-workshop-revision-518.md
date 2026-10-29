# 0360. Write revision 518 of the Photon Workshop container, without its second preview

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

The Photon Mono M5s, M5s Pro and Mono 4 read the same table container as the rest of the
Anycubic line, under `.pm5s`, `.m5sp` and `.pm4n`, at revision 518. `format-anycubic` wrote up
to 517, so the three machines could not be shipped.

Two sources describe 518 and agree where both speak. An open-source reader of the container
names its tables and fields: two more addresses in the file mark, a 96-byte header ending in
an intelligent-mode word, a 224-byte machine block, a `SUBIMGS` table repeating the layer
table's address, length and lit pixels, and an optional second preview. The files the
machines read settle what the reader leaves open: it writes 518 for
`.pm5s` and for any extension not listed, states fifteen machine properties, gives the
`SUBIMGS` table a 24-byte head and 44-byte rows, and carries no second preview at all.

No file a machine accepted has been read, and none of the three machines is in the room.

## Decision

`AnycubicVersion::V518` is written as both sources lay it out: eleven tables, a 64-byte mark
whose second-preview address is zero, intelligent mode off, the machine block's panel fields
filled from the profile, and the `SUBIMGS` rows filled from the same layer data as the first
table. `.pm5s`, `.m5sp` and `.pm4n` are extensions of the one container whose newest revision
is 518, and each of the three machines has a profile stating it. The details are in
`docs/formats/anycubic.md`.

## Consequences

Three more machines are in the catalogue, written in the layout these machines read. The
reader opens 518 for checking what was written, as it does the other revisions. A wrong
guess shows on the machine: a file it refuses or one it shows without a preview. The signal
to revisit is a printed file for one of these machines read byte by byte, or a machine that
refuses ours.

## Alternatives considered

### Write the second preview as well

The open-source reader writes it when a second thumbnail exists. It lost because no file for
these machines carries one, so a machine has no file with one to have been tested against, and its
size is stated two ways in that reader, 320 or 330 pixels wide.

### The option that won, and what it costs

The layout is right by two static readings, not by a print. Until somebody slices for one of
these machines and it prints, the three profiles carry a revision that has never reached
firmware from this writer.

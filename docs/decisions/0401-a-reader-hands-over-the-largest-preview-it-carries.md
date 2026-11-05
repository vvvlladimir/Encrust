# 0401. A reader hands over the largest preview a file carries

- **Status:** Proposed
- **Date:** 2026-11-05

## Context

Nearly every container this workspace reads carries one or more preview pictures, but
`SlicedFile` has had no place for them. So `convert` writes the blank image of the right size
the header still points at, and `encrust info` cannot show what a file looks like. Writers
already cut every record they need from one `Thumbnail` of `SOURCE_PX` (ADR 0048).

## Decision

`SlicedFile` carries `preview: Option<Thumbnail>`: the largest picture of the plate the file
holds, decoded to plain RGB when the file is opened — the second `.pwsz` picture is a side
view in known files, so there the first is taken. `None` means the container has none (`.cws`)
or its preview does not decode: a broken picture never stops a printable file opening, so a
reader turns that error into `None` where it reads the preview. The codecs two containers
share live in `core-format` — raw two-byte pixels, the run-length RGB15 record, a PNG entry
of an archive — and the readers' fuzz targets reach them through `open`.

A preview is read as the container states it. Known `.cxdlp` version 3 and `.cxline` files
set the high byte of every lit word, so their pictures come back red; nothing in the file
says what the colour was.

## Consequences

`convert` can hand the picture to the new file's writer, which cuts its own sizes from it as
it does from a rendered thumbnail, and `info` can write it out. Opening a file costs one
extra small decode, at most `MAX_PREVIEW_PX` pixels, which a header cannot talk past. The smaller
previews are dropped: a writer reduces from the largest anyway. Reopen if a caller needs a
container's exact records, a lossless round trip of one format for instance.

## Alternatives considered

### Every preview, as a list

Keeps each record as the file holds it. Rejected: no caller wants more than one picture, and
a list makes every caller pick, each in its own way.

### A method on `OpenFile` decoding on demand

Spares the decode for callers that never look. Rejected: the decode is small next to the
layer table opening already reads, and a trait method is one more thing sixteen readers
implement for the same answer.

### The decision that won, and what it costs

A field filled at open time is paid by every opener, and a preview that fails to decode is
silently absent rather than reported.

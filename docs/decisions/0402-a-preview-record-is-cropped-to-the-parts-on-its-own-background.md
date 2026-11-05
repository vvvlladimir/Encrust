# 0402. Crop a preview record to the parts, on the record's own background

- **Status:** Proposed
- **Date:** 2026-11-05

## Context

ADR 0048 renders one 512-pixel square and pads it into each preview record with the record's
background. Two things show on every printer's screen as a result. The render's own
background, a dark grey, is not the record's: a 4:3 record shows a grey square between black
bars, and a `.pwsz` one a grey square between teal bars. And a square fitted into a 4:3 or
16:10 record leaves a quarter to a third of its width empty, so a wide part is drawn smaller
than the record allows. Known files from vendor software crop the picture to the part and
fill the record with it.

The records are fixed by each format, 116 x 116 up to 800 x 480, and the writers already cut
every one of them from the job's single `Thumbnail` with `fitted_to`.

## Decision

A rendered `Thumbnail` carries a coverage mask, the pixels a face wrote into the depth
buffer. `fitted_to` crops a render to the box around its covered pixels, scales that box to
the record less a 6% margin, keeping its aspect ratio, and counts every uncovered pixel as
the record's background. A picture read from a file has no mask and is fitted whole, as
before.

The render is 1024 pixels square. It frames the parts to 88% of that, about 900 pixels, more
than the longest record side (800), so a record stays a reduction.

The writers, `PrintJob` and the formats' backgrounds do not change.

## Consequences

Every record of every format fills its own shape with the parts, on one background, and the
writers needed no edit. A converted file's preview, read from another file, is fitted as it
was.

A render costs more: a 978k-triangle sphere renders and is cut to 400 x 300 in 0.69 s rather
than 0.50 s on an Apple M1, once per exported file. The mask is a byte per source pixel,
1 MB, for the life of the job.

The `.pwsz` records are padded with the colours `docs/formats/pwsz.md` states, so a
`.pwsz` preview is now the parts on teal or navy throughout. Known files leave those pixels
transparent; writing an alpha channel is what would change that.

Reopen this if a record ever exceeds 900 pixels a side: the render then has to grow with it.

## Alternatives considered

### Render once per record, at the record's shape

It frames each record exactly and needs no mask. It lost on cost and on reach: four renders
per file and a camera fitted four times, as ADR 0048 already weighed, and `PrintJob` would
have to carry the plate rather than a picture, which `core-format` must not know.

### Render at the parts' aspect ratio instead of a square

One render, no mask, and the parts fill a record of the same shape. It still pads a record
of any other shape, still puts the render's background into the file, and changes the size
of the picture the window's recent-files list takes from the same renderer.

### The option that won, and what it costs

A mask beside the pixels, a crop that only a render takes, and a render four times the area
of the old one for a gain only the larger records show. A picture read from a file keeps its
own bars and background, since nothing marks which of its pixels are the part.

# 0269. Orientation never worsens a file, and a fold is noted rather than asked about

- **Status:** Accepted
- **Date:** 2026-10-14

## Context

The calibration part of ADR 0205 came back, and nothing in it is broken. Welded, it has no
boundary edge, no degenerate face, no duplicate. As written, four of its edges are walked
twice the same way, at one spot 0.1 mm across where two bodies' coplanar walls lie folded
onto each other. No turning of faces mends that.

`orient_outward` walked faces breadth first and gave up on a conflict, having already
turned faces on the way to it: 14 faces turned, four bad edges became twelve. The window
then called the model broken, asked about it on import, and its repair cut faces out and
patched the holes. The fill rule never needed any of it. The rasteriser counts a pixel solid
where the winding is positive (`docs/design/rasterisation.md`), so it takes what each body
encloses whichever way their surfaces are zipped together.

## Decision

**`orient_outward` turns patches, not faces, and never leaves more conflicts than it
found.** Faces already agreeing with each other form a patch, which is turned as a whole,
so a conflict inside one stays where the file put it. Between patches, seams are honoured
heaviest first through a union-find that keeps parity. Each group turns whichever half
holds fewer faces, then each closed shell is turned outward by its signed volume as before.
If the result leaves at least as many conflicting edges as the file had, the file's
winding stands. `Orientation::orientable` now means no conflict is left. The struct is
unchanged, so the project format is too.

**`MeshDiagnostics::is_sound` is no boundary edge, no degenerate face and no duplicate.**
An unbalanced edge with no hole behind it no longer makes a model unsound. `is_closed`
keeps its meaning from ADR 0205. The window lists such edges as a note under the quiet
mark, with no question asked and the section cap drawn. The command line prints a `note`
line, and `--strict` passes the model.

## Consequences

The calibration part opens with nothing turned, nothing asked and nothing taken away. Its
section cap is counted from crossings that are locally wrong across 0.1 mm, which is the
cost ADR 0205 declined to pay. It is paid here because the repair it chose instead cut real
surface out of a sound model.

There is no threshold: any number of conflicts that orientation cannot mend is a note. The
signal to reopen this is a file whose conflicts print wrong, rather than one that only
reads oddly.

## Alternatives considered

### Keep the face walk and revert when it worsens

This is the smallest change. Rejected because the walk still turns faces that agree with
everything around them whenever a conflict lies on their path. Reverting saves the file
only when the total gets worse, not when the walk merely moves a conflict around.

### The option that won, and what it costs

A seam between patches is honoured by weight, which is a heuristic: on a surface where no
orientation exists, it is not the fewest possible conflicts, only never more than the file
had. And "sound" no longer implies "closed", so a reader of the diagnostics has one more
distinction to hold.

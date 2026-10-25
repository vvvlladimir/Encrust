# 0351. Repair has methods, and drops open specks but never a closed shell

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

Repair (ADR 0270, 0349, 0350) was one fixed chain. A user who wants only the faces a broken
export dropped put back, or only the winding fixed, had to take everything: collapsed faces,
stitched seams and patches across every hole, however large.

Exported files also carry specks: a stray triangle or a scrap of open surface floating apart
from the model. Repair patched each into a tiny closed shell, which then sliced as a dot of
resin. A rule that drops every shell under an area also drops a thin closed pin that is part
of the model, with nothing to tell the user.

## Decision

**`mend` takes a `MendMethod` and `MendSettings`.** `Repair` is the whole chain, the one a
Repair press runs. `CloseHoles` drops the doubled faces and patches every hole, without
collapsing, stitching or dropping specks. `CloseSmallHoles` patches only the loops of three
corners a single missing face leaves, through `fill_holes_within`, and turns nothing.
`Turn` only orients. Every method keeps the never-worse rule. The command line names one as
`--repair=METHOD`; the window runs `Repair` until the Repair tool offers the others.

**`Repair` drops a speck: a shell with an edge an odd number of faces use, whose faces add
up to under 1 mm²** (`DEFAULT_SMALLEST_SHELL_MM2`, `--smallest-shell`). It runs after
stitching, so a scrap a seam joins to the model is kept, and before the fill, so a speck is
not closed into something that looks kept. A closed shell is never dropped by area.

## Consequences

- The command line can repair as narrowly as it is asked; the window gains the same choice
  with its tool.
- `Mended` reports `shells_dropped`, and its orientation is `None` for a method that does not
  orient. The stage notice, the text report and the JSON `shells_dropped` say what went.
- On the test models no shell is dropped: none of their open shells is that small.
- A model made only of specks comes back as it was, since a repair leaving nothing is kept as
  found.
- The signal to revisit is a real open part under 1 mm², or a closed speck that slices as
  noise.

## Alternatives considered

### Drop every shell under 1 mm², closed or not

The simplest rule. Rejected because it removes a closed pin of a real model silently.

### Drop a closed shell whose volume is under a limit

Would catch a closed speck too. Rejected for now: the volume limit is a part setting of
`core-volume`, which `core-geometry` cannot read, and a choice about a closed shell is the
user's, which the Repair tool's shell list will give them.

### The option that won, and what it costs

An enum switch over four methods inside one function, and an area limit that is a length
squared rather than a fraction of the model, so a model drawn very small loses open parts
that a larger copy keeps.

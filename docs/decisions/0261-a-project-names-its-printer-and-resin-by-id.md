# 0261. Name a project's printer and resin by id, give the user's own ids a random tail, and picture the plate in the file

- **Status:** Accepted
- **Date:** 2026-10-14

## Context

A `.encrust` file of version 4 carried its printer and resin whole beside their catalogue
ids (ADR 0191), and opening it put both in the window. A project passed from one user to
another therefore brought somebody else's machine and exposures with it, and the slicing
saved with them, over the opener's own calibrated profiles.

The id alone was no safer: the user's own profiles were named for their file stem,
`my-saturn`, so two people's printers of one name shared an id, and a project would land on
the wrong machine. Nothing ships a resin (ADR 0196), so every resin is one of these.

A file also had no picture of its own: the start page draws one per path into a folder of
the user's (ADR 0234), which a file moved to another computer leaves behind.

## Decision

- **By id only.** `printer` and `resin` in `project.json` are catalogue ids, never profiles.
  The window takes each where its catalogue has that id, and the plate's slicing only
  together with the printer; otherwise it keeps its own printer, resin and slicing, opens
  the plate all the same and says which it kept. `core-engine` opens a plate on a `Machine`
  the caller resolves: `encrust-cli` from the catalogue or `--printer`/`--resin`,
  `web-engine` from the printer and resin TOML handed to `slice`.
- **Own ids.** A printer or resin the user makes, copies or imports is named
  `<file stem>-<8 hex digits>`, the tail drawn from `getrandom`. A shipped id stays
  everyone's. At start-up the window gives every bare one of the user's printers and resins
  a tail once, rewrites the resins' tuning keyed by the printers that moved, and moves its
  own preferences after them.
- **Version 5.** Version 4 is still read, keeping only the ids it carried.
- **A picture.** The window writes the start page's 480 by 320 picture of the plate as
  `preview.png`, stored rather than deflated, and takes it as the start page's tile on
  opening rather than drawing it again.

## Consequences

- Opening a project never changes the profiles or slicing of whoever opens it; a shared
  project slices on the opener's own machine and resin.
- A project from another person almost always opens on the window's own printer and
  resin, since their ids are theirs. The command line refuses such a project until a flag
  names a printer and resin, and `web-engine` always needs both handed over.
- Version 4 files naming a bare id that start-up has since renamed no longer find it.
- `printer-profiles` depends on `getrandom` 0.3, already in the tree under `eframe`, and
  turns on its browser source itself so that `web-engine` builds without `encrust-web`.
- Saving draws the picture on the window's thread, before the file is written.
- Reopen if users ask to take a project's printer whole, which would want an explicit
  import rather than a silent one.

## Alternatives considered

### Keep carrying the profile, and use it only where the id is missing

What version 4 did. It still writes a stranger's machine into the window, and keeps the
file the size of every profile it names.

### Random ids without a readable stem

A UUID alone is just as unique, but the user's profile directory would no longer say what
each file is.

### The option that won, and what it costs

A project is no longer self-contained: slicing it anywhere but its maker's window needs the
printer and resin found or named again, and renaming the bare ids breaks every older
project naming them.

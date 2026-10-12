# 0233. Make Settings a window like Machine and resin, and carry a profile out as its file

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

After ADR 0220 the Settings screen held two pages, Supports and Updates, and still replaced
the plate to show them: a list of pages, a list of profiles and a form, in a layout of its
own. Machine and resin, the window beside it, is a modal with a title row, a list down the
left, a header, a table and an action bar. The two looked like different applications.

Nothing could be carried between installs or users: no machine, resin, support profile or
tool preset could be written out or taken in, short of finding the profile directory by
hand. The File menu loaded a printer or resin from a file for that session only, with no id
to remember or tune by. The Hollow
and Drain presets of ADR 0226 could only be deleted from the tool's row, and a few things
about the window itself had nowhere to be set.

## Decision

**Settings is a modal over the plate, in the same frame as Machine and resin** —
`panels/settings/frame.rs` draws both: the title row, the list down the left, the header,
the scrolled body, the action bar and the table. Its pages are **Supports**, **Hollow**,
**Drain**, then **General** and **Updates**, then links to Machine and resin and the sheet
of keys. A page of profiles is a table with Edit, Duplicate, Export and Delete along the
foot, the way Resins is; a support profile's form opens in the table's place. Hollow and
Drain presets are renamed, exported and deleted here and still edited in their tool.
Deleting is asked about first, as everywhere else (ADR 0196).

**A profile is exported as the TOML it is kept as** in the user's directory, named
`<id>.toml`, and imported from the page it belongs to: Import over each Settings table,
beside Add a machine and beside Add a resin. An import never overwrites: it is saved under
an id of its own, with " (imported)" after its name when that name is taken. A resin is set
up on the machine the window has open. The File menu's Load printer and Load resin profile
become **Import a machine** and **Import a resin**: the same import, then the profile under
the plate, the resin on the machine in hand. A preset has to read as that tool's settings, or it
is refused. The work is `encrust-app`'s `exchange.rs`, over the catalogue's existing calls.

**General** holds what is set once: whether a run opens on the start page, the direction
of the wheel's zoom, the folder the sliced-file dialog opens in, and where the profiles are
kept, with a button that opens it. They are written to `preferences.json` under `general`.

## Consequences

- The plate stays in view under every settings page, and the two windows change together
  because they are one piece of code.
- A user moves a measured resin or a machine to another computer, or hands it to someone,
  as one file anybody can read. The catalogue's format is now one users hold copies of, so
  a change to it has to keep reading what older releases wrote; it already had to for the
  user's directory.
- A profile can no longer be used without being kept: a file opened once to try it lands
  in the catalogue and has to be deleted there.
- A backup of every profile at once is not offered: export them one by one, or copy the
  folder General opens.

## Alternatives considered

### Keep the screen and restyle it

The pages would match, but the screen still hides the plate, and its layout would be a
second copy of the window's to keep in step.

### A container of its own for exported profiles

A header naming the kind would let one Import button take anything. It adds a format to
version for what the TOML already says, and the page an import is started from already
knows the kind.

### The option that won, and what it costs

The window is 1080 by 720 points at most, so a support profile's form has the right side's
width and not the screen's. Settings and Machine and resin are both modal, so going from one
to the other closes the first.

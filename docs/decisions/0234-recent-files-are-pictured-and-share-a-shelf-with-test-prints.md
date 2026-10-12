# 0234. Show the recent files as pictures, and switch between them and the test prints

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

ADR 0232 gave the start page a row of test prints, each a rendered picture, a name and two
lines, under the drop zone and a list of recent file names. A returning user comes back for
their own files, which the page showed as bare names, while four tests most users print once
took the most room. A file is found faster by what it holds than by what it was called.

## Decision

The drop zone is a strip across three of four columns, with **Open a model** and **Open a
project** at its right. Under it is one shelf of tiles, four to a row: a picture, a name and
two words under it. It shows **Recent** — up to the eight files `recent.rs` keeps, with how
long ago and the kind of file — or **Test prints**, pictured from their meshes as ADR 0232
had them. The fourth column is a card naming the shelf not shown, and a click shows it;
which one is shown lasts the window's run and starts on Recent.

A picture is the plate as it stood when the file was opened or saved: a model's own mesh
once its import lands, a project's visible models on the plate shown. `core-thumbnail`
renders it off the drawing thread at 480 by 320 pixels, and it is written as a PNG named by
the FNV-1a hash of the path into `recent/` beside `preferences.json`
(`recent/picture.rs`); pictures of files the list no longer holds are deleted after each
write. A file with no picture shows a glyph. A model enters the list when it reaches the
plate rather than when it is asked for, so one that fails to open is not listed.

## Consequences

- The page opens with no file read but the PNGs, a few tens of kilobytes each.
- A picture is as old as the last open or save: a model edited elsewhere keeps its old one
  until it is opened again.
- The tests lose their two lines on the page; a tile says what its test tells when the
  pointer rests on it, as **File → Open test print** does.
- The tests are one click further away than in ADR 0232; reopen if first runs miss them.

## Alternatives considered

### Read each file again when the page is drawn

Always current, but a model of millions of faces is seconds of reading on every start, and a
project needs its whole archive opened for one picture.

### The test prints as a menu off a folder card

Tried first: smaller, but four names say less than four pictures, and the menu hid the
tests behind a second click that a shelf of their own does not need.

### Keep the picture inside `preferences.json`

One file instead of a folder, but a base64 image per entry rewritten on every preference
change.

### The option that won, and what it costs

A second folder of the user's files beside the preferences, which names nothing new but
shows what their models look like; and a picture rendered on every open and save, a thread's
work proportional to the faces on the plate.

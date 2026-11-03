# 0394. A test print too large to bundle is fetched from the repository on its first click

- **Status:** Proposed
- **Date:** 2026-11-02

## Context

ADR 0232 builds the test prints from code and bundles nothing else. Crustling, the
project's mascot, is a sculpt: 174,942 faces whose toes, crystals and pitted shell are what
a user prints it for. As an STL it is 9 MB; packed losslessly it is still about 2 MB in
every installer and in the browser build, for one figure most users open once, and
simplified to fit, it loses the detail that makes it worth printing.

## Decision

A test print too large to bundle lives in the repository's `test-prints/`, as the STL a
user would download by hand; Crustling is the first, scaled to 30 mm across and otherwise
as sculpted. The window bundles only its picture, `assets/samples/<name>.png`, kept as
shade and coverage like a recent file's (ADR 0385) and tinted in the tile; a test renders
it from the model and fails when the two drift apart, retaken with `UPDATE_SNAPSHOTS=1`.

Its tile comes before the bundled tests' and looks like theirs. The first click fetches it
from the repository's `main` branch on a thread, through the counted fetch that downloads
an update, into `test-prints/` beside `preferences.json`; while it comes, a bar runs along
the picture and the tile tells the share; the window polls it every frame, whatever is
shown. The next click opens it from disk. A failed fetch says why on the status line and
leaves the tile to be clicked again. A browser, which keeps no files and
whose `ureq` cannot run, opens the file's address in a tab instead.

## Consequences

- The installer grows by a 48 KB picture; the model costs nothing until it is asked for.
- The first click needs the network; a link in the window depends on the file's path in
  `main`, so `test-prints/` is not renamed without a release.
- The command line has nothing to add: the file is a plain STL in the repository.
- The File menu's Test prints lists it first, before the four drawn in code, its state as the row's
  keys, and a click there does what a click on the tile does.
- Reopen if a second large test print is wanted offline, or the repository moves.

## Alternatives considered

### Bundle the sculpt, packed losslessly

The meshoptimizer codecs over integer corners and deflate: 2.1 MB in every build, and a
picture rendered from half a million faces on the drawing thread.

### Bundle it simplified

20,000 faces in 179 KB, but the toes and the shell's texture blur away.

### Open the file's page in a browser

No fetch code, but the model would land in the browser's downloads, not on the plate.

### The option that won, and what it costs

A fetch, its progress and its failure in the window for one tile, a picture that a test
has to keep honest, and a test print that works only once the network has been there.

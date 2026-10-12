# 0232. The start page offers resin test prints of its own, and leaves the machine to the top bar

- **Status:** Accepted; the row of tests under the recent files is superseded by 0234
- **Date:** 2026-10-12

## Context

ADR 0220 put the machine the plate prints on beside the start page's drop zone, over the
recent files. The top bar's chip says the same machine and opens the same window, so the
card repeated it, and the two stacked cards never matched the zone's height. A new machine
or a new resin is first tried on a known test, which a user had to go and find elsewhere.
The tests that are easy to find are mostly made for filament printers; the resin ones carry
licences that ask for credit or forbid changes, or none at all, and a downloaded mesh runs
to megabytes in every installer and in the browser build.

## Decision

The start page drops the machine card. The recent files stand beside the drop zone at its
height, and under both a row of **Test prints**, each a picture, a name and two lines on
what it tells; a click opens it as if its STL had been dropped. The File menu offers both
too, once the page is gone: **Open recent** under Open project, which a browser leaves out
for want of paths, and **Open test print** under Open model. The four tests are the
project's own, drawn in code in `encrust-app/src/samples/` and covered by its licence:

- **Detail test**: spikes narrowing to 0.05 mm, ledges at 20°, 30° and 45°, no supports.
- **Exposure check**: pins, fins, slots and blind holes from fine to coarse.
- **Surface finish**: a dome, a column and a 5° ramp.
- **Fit gauge**: a 6 mm hole and pins from 0.3 mm under to 0.1 mm over it.

Each carries the **mark built as the solid the logo draws**: three slabs with a gap between
each, the top one cut into the letter, so that from the thumbnail's corner it is the logo.
The letter is the logo's outline taken back off the isometric to a flat 50-point polygon.

A model is put together from four primitives that need no boolean operation — a prism of a
simple polygon, a prism along Y, a rectangle with a round hole, and a profile turned about
the vertical — and parts overlap where they meet, which the slicer's positive winding unites
(ADR 0071). Its picture is rendered by `core-thumbnail` from the same mesh, at the card's own
size the first time the page is drawn.

## Consequences

- Nothing is bundled but code: the four together are about 21,000 faces, built in well
  under a millisecond each, and their pictures take about 20 ms once a window.
- A picture cannot drift from its model, and a test's sizes are constants beside their
  meaning.
- A shape is changed by changing code, and a new primitive needs its own closed-solid test.
- Reopen if a test is wanted that the primitives cannot draw without a boolean operation.

## Alternatives considered

### Bundle freely licensed downloads

Tried first: four public-domain or CC BY resin tests, welded into 3MF, came to 3 MB with
pictures committed beside them, one author to credit wherever the window ships, and meshes
that needed repair before they opened sound.

### Draw the tests once and commit the files

Small files too, but a generator to keep beside them, and pictures that drift.

### The option that won, and what it costs

Some 800 lines of geometry in the window crate, tests and the letter's table included, for
four objects most users print once; and a test is only as true as the code that draws it,
which a printer can still prove wrong.

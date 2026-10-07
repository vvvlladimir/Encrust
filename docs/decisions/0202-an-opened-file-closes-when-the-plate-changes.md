# 0202. An opened sliced file closes when the plate changes

- **Status:** Accepted
- **Date:** 2026-10-07

## Context

ADR 0151 gave the preview two sources and said a file is never stale: nothing about the
scene makes a container another program wrote out of date. That holds for the file's own
layers, and it left a window stating two stacks at once. A `.goo` open under the slider and
a model then imported showed the file's mask and the file's panel beside the plate's
estimate — 200 layers in the heading, 1710 in the footer — and the layer section took its
exposure from the window's resin rather than from the file's own table.

The plate is what the window is for; a file is looked at beside it. Only one of them can be
under the slider, and whichever it is has to answer every reading on the screen.

## Decision

A file is shown over the plate it was opened above, and closes the moment that plate
changes. `Preview` notes what stood on the plate when the file was opened — the half of
`stack_fingerprint` that is the scene, without the numbers it would be cut with — and the
window closes the file when the two differ, saying so in the status bar. Changing the layer
height or the compensation is not a change to the plate and leaves the file open.

Every reading of a file comes from the file: the layer's exposure is its own table's, and
the section carries no heading from a resin profile that never wrote it.

## Consequences

Importing a model, arranging the plate, deleting an object or moving one puts the plate back
under the slider instead of mixing it with a file. The file has to be opened again to be
looked at further, which is one click and no state to keep.

ADR 0151's "a file is never stale" stands for what it meant — a file is never re-cut and
never measured — and is amended in what ends its turn.

## Alternatives considered

### Mark the preview as showing a file and leave it open

The heading already names the file. The readings that mixed sources came from the panels
below it, so labelling the source harder would have left the wrong numbers on the screen.

### The option that won, and what it costs

A plate fingerprint is taken every frame while a file is open, which is a walk over the
objects on the plate and no geometry. The cost is that a change the user thinks is unrelated
— nudging a model on the plate — closes the file without being asked.

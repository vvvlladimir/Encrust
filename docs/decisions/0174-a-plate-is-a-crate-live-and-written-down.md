# 0174. A plate is a crate, live and written down

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

ADR 0127 gave the write stage its own crate and deliberately left what sits above it in
the front ends, because orient, hollow and supports were not duplicated. Two things since
then were.

The step from *a plate the user arranged* to *the file a printer reads* was written twice:
`encrust-app/src/job/{merge,pipeline}.rs` baked the visible objects into one mesh with the
resin's shrinkage applied, worked out the windows, rendered the thumbnail, assembled the
`PrintJob` and called `core_pipeline::write`; `encrust-cli/src/{pipeline,slicing,sliced_file}.rs`
did the same five things in its own shapes — `compensated`, `Plan`, `write_sliced`. A
browser front end would have been the third copy.

The `.encrust` project file was the other: the manifest, the mesh blobs and their
(de)serialisation lived in `encrust-app/src/project.rs`, so nothing without a window could
open a plate — not the command line, not a browser.

## Decision

A new `core-engine` crate owns the plate, in both the form it is cut from and the form it
is written down as.

- `Plate` is a list of `Model`s — a mesh, where it stands, the cuts wound into it and the
  supports under it — plus the printer, the resin, the panel overrides, how the stack is
  cut and which container is written. `Run::of(&Plate)` bakes, plans the layers and builds
  the header; `Run::write`, `Run::write_file` and `Run::measure` are what a front end
  calls.
- `core_engine::project` is the `.encrust` format: `Manifest`, `Project`, `read_from` and
  `write_to` — data and (de)serialisation, and the plain types the file carries, `Axis`,
  `Keep` and `Array` among them. No `Scene`, no `History`, no dialog.

The crate sits above `core-pipeline` and takes `format-*` only through it. It still does
not drive orient, hollow or supports: as in ADR 0127, those are the front end's to run,
and a `Model` carries their *result*. It starts no thread.

## Consequences

One path now bakes a plate and streams it, so the window, the command line and a browser
cannot disagree about what a plate prints as. The window's half becomes one conversion,
`models_of`, and the command line loses `compensated`, `Plan` and its own `PrintJob`.

`core-engine` is now the widest crate in the graph, and the pressure that fell on
`core-pipeline` falls here instead: it must stay *a plate in, a file out*, and a thing two
callers merely happen to share still does not belong in it.

Reading a project no longer needs a window, which is what `encrust slice plate.encrust`
and a browser both wait on. The window keeps the dialogs, the digest check and the
`Scene` ↔ `Manifest` conversion, because all three are its own.

If a third front end never arrives, the crate is still two callers sharing one loop — the
same bet ADR 0127 took, with the duplication it removes measured in the same way.

## Alternatives considered

### Widen `core-pipeline`

It is the write stage by ADR 0127 and says so in its own module doc. Baking a plate,
rendering a thumbnail and holding a project format would make that sentence false, and the
next shared thing would land there for the same reason.

### Two crates: the run and the project format

Considered and rejected. `Plate` and `Manifest` are the same facts in two forms — the same
`Transform`, the same `ModelHollow` and `ModelSupports`, the same slicing settings — so the
format crate would depend on the run crate for its types and have no second caller to show
for the split.

### Let the engine own orient, hollow and supports as well

The plan's first sketch, and ADR 0127's answer still holds: the command line drives them
from flags and the window from tool state, so there is no shared shape to move yet. When
a headless caller needs the whole sequence it is one function over these stages, not a
redesign of them.

### The option that won, and what it costs

A crate that spans nine others, and a second place — after `core-pipeline` — where a
reader has to ask "is this the write stage or the run above it?". The line is that
`core-pipeline` is handed a mesh and windows, and `core-engine` is handed a plate.

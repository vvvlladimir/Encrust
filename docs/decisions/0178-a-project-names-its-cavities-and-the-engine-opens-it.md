# 0178. A project names its cavities, and the engine opens it into a plate

- **Status:** Accepted
- **Date:** 2026-10-03

## Context

ADR 0097 keeps only what the user asked for in a `.encrust` file and rebuilds the rest on
load. For supports that holds: the points are in the file and the trees regrow from them.
For a cavity it did not: the file kept the blockers, holes and channels, but not whether
the model had been hollowed at all, so an opened plate came back solid until the Hollow
button was pressed again — and was saved solid if it was not.

A browser has no window to press it in. Step A2 of the web work slices a project the
desktop saved, so the file has to say everything that is printed, and something without a
window has to turn it back into the `Plate` a run takes.

## Decision

- Each object's hollow state carries `cavity: Option<Cavity>` — the wall thickness, mode,
  precision and infill the shell was built with. Its own blockers and channels are already
  beside it, so nothing else is needed to build the same cavity again. The manifest is
  version 2; a version 1 file reads with every model solid, and an older build refuses a
  version 2 file rather than dropping the cavity silently.
- `core_engine::open_plate(&Project, &Opening) -> Plate` turns one plate of a project into
  a run's input: the resin tuned to the printer and carried to the project's layer height,
  every cavity built again with `core_volume::hollow`, every hole cut and every support tree
  grown from its points. `Opening` holds what the file does not say: which plate, the
  memory one cavity may take, the raster window and the time to stamp.
- The window writes the cavity on save and, on opening, starts the hollowing run it would
  have started from the button, one model at a time with each model's own wall.

## Consequences

A plate opens as it was saved, in the window and anywhere else, and `encrust slice
plate.encrust` (step B) has its entry point. Opening a hollowed project now costs a
hollowing run, which is seconds on a real model; the window runs it on its worker and
counts the plate as saved in the state it reaches once the run is done.

What `open_plate` does is the window's `Slicing` and `models_of` restated over the
manifest. If the two drift — the window tunes a resin differently, or groups supports
differently — a project slices differently in a browser than at the desk; the tests on
both sides pin the same cases.

## Alternatives considered

### Keep the shell mesh in the file

Exact, and nothing to rebuild. But a shell is the model again plus its cavity, so a file
doubles for every hollowed model, and ADR 0097's line — what was asked for, not what was
built — would have its first exception.

### Rebuild the plate in the browser front end

The browser would read the manifest and drive `core-volume` and `core-supports` itself.
That is the third copy of the orchestration ADR 0174 exists to prevent.

### The option that won, and what it costs

`core-engine` now drives hollowing and support growth for one caller shape, the project
file, against ADR 0174's line that it is handed results. It is bounded by being only what
a saved plate already says, but it is a second statement of the window's own rules.

# 0260. Keep cavity fields in the window, to a fixed budget, so cuts re-carve without a build

- **Status:** Accepted
- **Date:** 2026-10-14

## Context

A blocker placed or a channel dug on a hollow model rebuilds its shell at once (ADR 0076).
Neither is in the cavity's field: blockers are taken out of it after the build, and infill
is cut from it after that. Yet every rebuild built the field again, which is nearly all of
a run — 5.4 s of 5.8 on a 406k-face bust at the default precision, 10.6 s at the finest.
Keeping the field costs memory the machine may not have: 26 MB for that bust at the default
lattice, 158 MB at the finest. In a browser, memory a page grows into is never handed back.

## Decision

- `core-volume` splits a run in two: `cavity_field` builds the field a model's cavity is cut
  from, `carve` cuts the shell from it with blockers and infill. `hollow_at_scale` is the two
  in a row, so its result is unchanged and the core keeps no state.
- `encrust-app` keeps the fields of the models hollowed lately (`job/hollow/fields.rs`),
  keyed by the mesh, the shape it stood at and the settings without blockers or infill,
  most recently used last. The fields together may hold **512 MB** on the desktop and
  **128 MB** in the browser. A field larger than that is not kept, and the longest
  untouched go first to make room.
- In the browser the kept fields are let go before a field is built and while a plate is
  sliced, so they never stand on top of those peaks.

## Consequences

- Moving a blocker or a channel re-carves in 0.4 s instead of 5.4 s on the bust above, and
  in 2.4 s instead of 10.6 s at the finest lattice, with the same shell to the byte.
- A machine that cannot spare the memory sees the old behaviour: a field over the budget is
  rebuilt every time, never held.
- With blockers, the unblocked field is now alive while the cavity is meshed; before, it
  was dropped once blocked. That peak is one field more.
- Reopen if a budget proves wrong in the field — machines swapping at 512 MB, or browser
  tabs killed — or if blockers are moved on models whose fields never fit.

## Alternatives considered

### A budget read from the machine's memory

A share of installed RAM fits each machine. It needs a new dependency for the desktop,
which has reported zero available memory on macOS, and the browser has no portable way to
ask at all: `navigator.deviceMemory` exists in Chromium alone and is coarsened.

### Rebuilding only the part of the field a blocker covers

`build` can clip to a box, and a blocker changes the cavity only near itself, so this would
cost no memory. The cavity mesh would have to be cut and stitched along the box, which is
a separate piece of work; it is the next step if the budget is too small to help.

### The option that won, and what it costs

Memory held between runs, up to the budget, and a cache to keep correct: a key that missed
a setting would carve a stale field. The key holds every setting a field is built from,
and `carve` takes the wall, mode and precision from the field itself rather than from the
caller.

# 0302. A shipped machine takes its locks from the build, and may lock its wait mode

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

ADR 0301 let a printer profile lock resin settings in `[locked]`. Two things it left open
showed on the first machine it was used for.

The window saves the whole profile when a user edits a machine, and that copy stands over
the shipped one by id. A copy saved before a lock never gets it: the Saturn 4 Ultra the user
had saved kept the peel locks of the day it was saved and missed every later one.

The vendor slicer offers the Saturn 4 Ultra and the Mars 5 Ultra one way of waiting, resting,
and leaves the three rests to the resin. `[locked]` could only hold `waits` whole, the rests
with the mode. A file we wrote for the machine in light-off mode was one the vendor never
writes.

A file the vendor slicer wrote for this machine from a preset carried over from another one
stated a real peel, 5 mm at 60 mm/min. Its hidden fields default to 0.05, which is what the
lock holds.

## Decision

- **A shipped machine's locks are the build's.** Loading the user's directory, and saving a
  printer, sets each copy's `locked` to the shipped profile's of the same id. A machine the
  user made, with no shipped id, keeps its own.
- **`PrinterTuning` carries `wait_mode`.** It is laid over `waits.mode` after `waits`, so a
  machine can hold the mode and leave the rests to the resin. The window offers no choice of
  mode where one is locked. A resin's own table never states it: `of_changes` leaves it out.
- `elegoo-saturn-4-ultra.toml` locks `wait_mode = "rest"`, and its peel stays at the hidden
  defaults, 0.05.

## Consequences

A lock added to the catalogue reaches every user at the next start, copy or not. A user cannot
lift a shipped machine's lock even by editing their copy: what the vendor will not let them
change, neither do we. A user who needs to has to save the machine under another id.

Whether the firmware moves by a peel the file states after its tilt is not yet measured. A
print of a file stating 5 mm settles it; the placeholders stay until then.

## Alternatives considered

### Merge only an empty `[locked]` from the shipped profile

Keeps a user's own locks on a shipped machine. It lost because an empty table is also what
every copy saved before ADR 0301 holds, and the two cannot be told apart.

### A separate locks type beside `PrinterTuning`

Would keep `wait_mode` out of a resin's table. It lost on cost: the table is shared by every
reader and the window, and a resin table with the key does no harm, it states what the resin
already holds.

### The option that won, and what it costs

A copy's `[locked]` is read and then ignored, so a hand edit there does nothing and says
nothing.

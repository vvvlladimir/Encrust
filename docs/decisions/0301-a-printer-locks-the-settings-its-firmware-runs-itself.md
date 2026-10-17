# 0301. A printer locks the settings its firmware runs itself

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

ADR 0142 cleared advance mode for the Elegoo Saturn 4 Ultra, so the machine reads the `.goo`
header alone. The header still carried the resin's peel, 6 mm at 65 mm/min, and on such a
file a user saw the plate rise again after the vat tilted.

The vendor slicer's machine definitions hide fields per machine and write a fixed default for
each: every peel field at 0.05 on the tilting vats (the Saturn 4 Ultra, its 16K, the Mars 5
Ultra), two of which are offered resting alone; the light power at 255 and the transition at
0 on older Chitu boards; light-off waiting on Anycubic and Halot machines; the bottom peel on
a Halot Mage. Files it wrote from fresh presets for the Mars 5 Ultra and the Saturn 4 Ultra
16K carry exactly those values. Our window offered all of them on every machine.

## Decision

- A printer profile carries `[locked]`, a `PrinterTuning`: any key of a resin's per-printer
  table, and `wait_mode`, which holds the mode and leaves the rests to the resin.
  `MaterialProfile::on(printer)` lays it over the resin last. A run, a conversion and the
  window's estimate take the resin through it; the form and the inspector leave out every
  locked field, and a card all of whose fields are locked. A locked speed of zero is refused.
- **A shipped machine's locks are the build's.** Loading the user's directory and saving a
  printer give each copy the shipped profile's table of the same id; a machine the user made
  keeps its own.
- The print time counts no travel under 1 mm/min, the placeholder floor of ADR 0293: that
  motion is the machine's own, and its time is the resin's measured layer time.
- What the vendor slicer hides, at its default, is locked for every machine of ours it matches
  by panel, in the profile file alone: `gen-profiles` keeps a generated file's `[locked]` when
  it rewrites the rest from the source. The Mars 5 Ultra and the Saturn 4 Ultra 16K ship,
  read off the vendor's files for them.

## Consequences

What the vendor will not let a user change, neither do we, and a lock added to the catalogue
reaches every user at the next start, copy or not. A resin keeps its own numbers for the
machines that do not lock them. Locking the transition at 0 removes a ramp we used to write
on machines that claimed the per-layer tables; the vendor writes none there.

A tilting vat's estimate leaves the tilt out until the resin's layer time is measured. The
vendor writes the bottom layers' light-off apart from the rest; one delay cannot say that.

Whether a tilting vat's firmware obeys a peel the file states is not measured: a preset
carried over from another machine made the vendor write 5 mm at 60 mm/min for one. Reopen
this if a Saturn 4 Ultra prints that file without rising twice.

## Alternatives considered

### A `tilting_vat` flag

ADR 0142 proposed it. It names one mechanism and fixes one set of fields in code, when the
evidence is machines hiding different fields at different values.

### A lock table in `gen-profiles`

Each lock would live twice, in the table and in the file it renders, with a test to keep the
two equal. The file alone is what the window and the engine read.

### Keep a user's own `[locked]` on a shipped machine

An empty table is also what every copy saved before this decision holds, and the two cannot be
told apart, so the copies would never take a lock.

### The option that won, and what it costs

A lock is read off another program's machine definitions and written silently, with no field
left in the window to show it is wrong; and a hand edit to a copy's `[locked]` does nothing.

# 0050. Keep one resin catalogue and retune it per printer

- **Status:** Accepted
- **Date:** 2026-09-20

## Context

Exposure is not a property of a resin. The same bottle of grey standard needs about 3.2 s
a layer on a Mars 3 Pro's LED matrix and about 2.3 s on a Saturn 4 Ultra's COB engine, and
a viscous ABS-like resin wants a slower lift on a machine with a stiff film than on one
that tilts. A resin profile that carries a single exposure is therefore only true on the
machine it was measured on.

The obvious fix is a resin set per printer: a directory of resins under each machine. With
five machines and four resins that is twenty files, nineteen of which repeat the same
density, the same name and mostly the same motion, and a correction to the resin itself
has to be made in five places.

## Decision

There is one catalogue of resins. Each resin file carries the resin — its name, density,
layer height and a baseline exposure — and then one `[printers.<id>]` table per machine
holding only the settings that machine changes:

```toml
name = "Standard grey"
exposure_s = 2.6

[printers.elegoo-mars-3-pro]
exposure_s = 3.2
bottom_exposure_s = 38.0
```

A tuning table may override any printable setting: the exposures, the bottom block, the
light PWM and delay, the lift and retract distances and speeds, and the layer height the
exposure was measured at. It may not override the resin's name or its density, because
neither changes with the machine. Every key is optional and unknown keys are rejected, so
a misspelled setting is an error rather than a silently ignored line.

`MaterialProfile::for_printer(id)` resolves a resin against a machine and is what the
slicer, the window and the file writers see; the tuning tables never reach a sliced file.
A machine the resin says nothing about gets the resin's own numbers, and both the CLI and
the window say so out loud, because those numbers are a starting point and not a
calibration.

## Consequences

Correcting a resin is one edit. Adding a machine is one printer file plus one short table
in each resin, and the catalogue test fails until every shipped resin carries one, so the
gap cannot ship quietly.

The window has to keep the resin twice: as loaded, tuning tables and all, and as resolved
for the printer in hand. Changing the printer re-resolves from the loaded copy, which also
means a layer height the tuning names replaces one the user typed. That is deliberate — an
exposure is only valid at the height it was measured at — but it is a surprise the panel
has to keep visible.

A resin is resolved against an id, so a printer opened from a file rather than picked from
the catalogue has no id and gets untuned numbers. If that turns out to be the common case,
the fix is to key tuning by something the profile itself carries rather than by its
catalogue id, and this gets revisited.

## Alternatives considered

### A resin directory per printer

`assets/profiles/resins/<printer-id>/<resin>.toml`, a whole resin per machine. Honest and
flat, with no resolution step to understand, and it is what the original plan for the step
said. Rejected because it multiplies every resin by every machine: the shared part of a
resin gets copied five times and drifts, and a new machine means writing four full files
rather than four four-line tables.

### One flat resin list and let the user retune by hand

Simplest possible data model. Rejected because it puts the burden exactly where the user
cannot carry it: the numbers that differ between machines are the ones that ruin a print,
and a new user does not know that 2.5 s was measured on somebody else's printer.

### The option that won, and what it costs

Inheritance, however shallow, means a number the user reads in the file is not necessarily
the number that is used. The picker and `--list-profiles` mark which machines a resin was
measured on, and `for_printer` is the only path to a usable profile, but "where did 3.2 s
come from" now has two places to look instead of one. The tuning struct also duplicates the
field list of `MaterialProfile`, so a new resin setting has to be added twice or it cannot
be tuned.

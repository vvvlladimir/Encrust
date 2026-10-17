# 0301. A printer locks the settings its firmware runs itself

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

ADR 0142 cleared advance mode for the Elegoo Saturn 4 Ultra, so the machine reads the `.goo`
header alone, and it called the lift and retract left in the per-layer records dead weight.
The header carries them too: the resin's 6 mm at 65 mm/min, 8 mm for the bottom. On such a
file a user saw the plate tilt the vat and then rise and come back down a second time, which
a vendor file for the machine does not do. That file states 0.05 in every lift and retract
field, distance and speed alike.

The vendor's own machine definitions show this is not one machine's quirk. They hide fields
per machine and still write a value for each: every peel field, at 0.05, on the Saturn 4
Ultra, the Saturn 4 Ultra 16K, the Saturn 5 Ultra and the Mars 5 Ultra; the light power, at
255, on older Chitu boards; the transition count, at 0, on others; the rests on the Halot
line. Our window offered all of them on every machine.

## Decision

A printer profile carries `[locked]`, a `PrinterTuning`: the same keys as a resin's
per-printer table. `MaterialProfile::on(printer)` lays it over the resin last, so it wins over
the resin's own numbers and its table for that machine. A run (`core_engine::Run`), a
conversion and the window's estimate take the resin through it; the settings form and the
inspector leave out every locked field, and a card all of whose fields are locked. A locked
value must load as a resin's would: a speed of zero is refused.

The print time counts no travel at a speed under 1 mm/min, the floor `stated_speed_mm_min`
already uses for a placeholder (ADR 0293). Such a motion is the machine's own, and its time
is in the resin's measured layer time.

`elegoo-saturn-4-ultra.toml` locks all seven peel fields at 0.05. Its `per_layer_settings =
false` stays.

## Consequences

Any setting of any machine can be held at what its firmware needs, by data in its profile.
No code names a machine or a mechanism. A resin tuned on a peel machine keeps its lifts for
that machine: a lock changes what is written, not what the resin holds.

A Saturn 4 Ultra estimate leaves the tilt out until the resin's layer time is measured, so it
reads short. A lock cannot be lifted in the window; a user's own copy of the profile can drop
it by hand.

Only the Saturn 4 Ultra is locked. The other machines the vendor hides fields on are
either not in the catalogue or are generated (ADR 0165), and the generator learns a lock
table of its own when one of them is locked. Reopen this if a Saturn 4 Ultra still rises
twice on a file with the placeholders.

## Alternatives considered

### A `tilting_vat` flag

ADR 0142 proposed it. It names one mechanism and fixes one set of fields in code, when the
evidence is machines hiding different fields at different values: every new case would be
another flag.

### Hide the fields and keep writing the resin's values

Leaves the second lift in the file. Hiding without a value is only half of what the vendor
does, and the half the machine does not see.

### The option that won, and what it costs

A lock is a value someone has to read off a vendor file or its machine definition. A wrong
one is written silently, with no field left in the window to show it.

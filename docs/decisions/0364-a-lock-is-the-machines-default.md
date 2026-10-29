# 0364. A printer's lock is its default, which a resin's table for it overrides

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

ADR 0301 laid a printer's `[locked]` over every resin last, as what the firmware runs itself.
Printed files say otherwise. For the Voxelab Ceres 8.9 the machine's settings hide a transition
of 8 and the file carries 0, the preset's value; for the PioNext D158 they hide a bottom lift of 7 mm at
420 mm/min and wrote the preset's 5 mm and 120. A hidden field is the default the machine's
own presets start from, not a value the file must carry. Our lock gave the Ceres an eight-
layer ramp the printed file does not have: 310 s against 244.

The other half of 0301 still holds: a resin carried over from a peeling machine onto a
tilting vat must not peel 6 mm.

## Decision

- A lock is the machine's default. `MaterialProfile::on(printer)` lays `[locked]` over the
  resin except where the resin's tuning table **for that machine** states the field.
- `for_printer` keeps the table it applied in `printer_table`, so a flattened resin still
  knows what it states; a starting point taken from another machine's table states nothing.
- Importing another program's resin for a machine states its value of every field the
  machine locks, as that program would print it.
- The wait mode stays locked: it is what the firmware can do.
- The form still hides a locked field; ADR 0301 is superseded.

## Consequences

A resin made for a machine prints as its preset reads; a resin carried over
from another machine starts from the machine's defaults, as a fresh preset would. A
user cannot set a hidden field from the window, only through an imported preset or a hand
edit of the resin's table.

## Alternatives considered

### The resin always wins

How a preset already behaves on its own machine. Our resins are shared across machines, so
a 6 mm peel tuned on a Mars would reach a Saturn 4 Ultra.

### Track which fields a resin's file states

Every field of a resin would carry where it came from, through every copy and edit.

### The option that won, and what it costs

One more field on a flattened resin, and an override reachable only from a table, never
from the window.

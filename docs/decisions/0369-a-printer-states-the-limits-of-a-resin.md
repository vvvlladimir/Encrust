# 0369. A printer states the limits of a resin's settings

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

The vendor slicer's profile for a machine gives every setting two spans: `min..max`, past
which its field takes nothing, and `minalarm..maxalarm`, past which the field is accepted but
framed in its error colour, two of them with a reason (a thin layer prints longer, few bottom
layers come off the plate). Neither stops a slice. Most spans are the empty `0.001..5000`;
about 90 machines state real ones: a layer of 0.01 to 0.2 mm, a lift of 8 to 20 mm, a speed
up to 2700 mm/min, an exposure of 0.5 to 10 s.

Our window held every machine to one span per field, set by hand: 1 to 1000 mm/min for every
speed, under what some machines take; and nothing told a user a value was past what the
machine's maker advises.

## Decision

- A printer profile carries `[limits]`, keyed as `[locked]` is, with `[limits.second_stage]`
  for the second stage. Each entry is `{ min, max, warn_min, warn_max }`, any left out; the
  bounds run in that order or the profile does not load (`ProfileError::DisorderedLimit`).
- `PrinterLimits::breaches(material)` is the one check: each setting past `min..max` is
  `Stray::Refused`, past `warn_min..warn_max` `Stray::Advised`.
- The window's field takes the machine's `min..max` where it states one and its own span
  elsewhere. A value that strays is boxed in `danger` with one line naming the span, and the
  maker's reason under an advice where there is one (`limited_row`).
- `encrust slice` writes the file as asked and logs one warning per breach: a resin file or
  a flag is a deliberate choice, and a run that worked keeps working.
- Shipped profiles take only shown settings the machine does not lock, and drop the empty
  bounds: 0, 0.001, below zero, 5000 and over, and a count's least of one.

## Consequences

The window no longer lets a user type a speed past 1000 mm/min only on machines whose maker
states more; elsewhere the old spans stand. A machine whose profile states nothing behaves
as before. Bounds are the maker's word, not a measurement: a machine whose firmware takes
more than its maker lets the field hold is held to the maker until its profile is changed.
Reopen if a print shows a bound refusing what the firmware runs well.

## Alternatives considered

### Keep the advised span only

Simpler, and enough for a warning, but the window could not hold a field to what the
machine takes, which is the half the vendor's slicer enforces.

### Refuse the slice on a hard bound in the CLI

Stricter, but a resin already outside a bound would stop slicing from scripts that work
today, for a bound that is the maker's choice of field range rather than a firmware fact.

### The option that won, and what it costs

Two places hold a value to the bound, the field and the log, and a value from a file can
still sit past it: the window shows it in `danger` until it is edited, and the CLI writes it.

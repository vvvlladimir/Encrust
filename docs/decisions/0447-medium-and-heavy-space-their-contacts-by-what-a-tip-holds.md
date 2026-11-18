# 0447. Space Medium's contacts 3.5 mm apart and Heavy's 4.5 mm, by what a tip holds

- **Status:** Accepted
- **Date:** 2026-11-16

## Context

ADR 0323 placed Light, Medium and Heavy contacts 1.8 mm apart and made the presets differ in
thickness alone. A tip's section grows with the square of its contact, 0.3, 0.6 and 1.0 mm,
so at one spacing Medium's tips carry a quarter of what Light's do and Heavy's a tenth. On the
bench's thirteen models the share of what a tip holds that it carries (`use p90`) was 0.30 to
0.48 for Light, 0.08 to 0.13 for Medium and 0.03 to 0.05 for Heavy, against a limit of 1.2
where the calibration coupon's plates still held (ADR 0424). Nothing sat between a tip at its
limit and one ten times over it, and Medium and Heavy took two to three times the contacts of
Auto.

## Decision

Medium places contacts 3.5 mm apart under a face looking straight down and 6.4 mm under one
leaning just past its angle; Heavy 4.5 and 8.2 mm. Light keeps 1.8 and 3.3 mm. The steep
spacing keeps ADR 0324's ratio to the flat one, to the tenth of a millimetre below. Everything else of ADR 0323 stands.

The numbers come from `support-lab --sweep` over the corpus, `--min-tip-mm 0.3`, then a run at
them: Medium's `use p90` is 0.28 to 0.48, 0.62 on the islands model, and it holds 11 models
against 10 at 1.8 mm; Heavy's is 0.16 to 0.29 and it holds 10 against 9. The models a preset
fails at every spacing fail for other reasons: off the plate, or no room for a head.

## Consequences

Medium takes 31 % of its former contacts and 60 % of its resin, Heavy 20 % and 45 %. Each
step up still carries no more per tip: Light 0.3 to 0.5 of what a tip holds, Medium about
0.4 with a wider tip, Heavy about 0.2. A part switched from Light to Medium now gets fewer,
thicker supports, not the same lattice thicker. Contacts further apart leave wider unheld rims between
them; the bench's rim gap and bare underside did not grow, and are what to watch. Reopen if a
print shows sag between Medium's or Heavy's contacts.

## Alternatives considered

### Build Medium and Heavy as Auto does, sized by what each tip holds

Auto already spaces and sizes by load. Making a hand-picked preset an automatic one with a
floor on its shape changes what the presets are and how the window shows their groups; a
larger change than the spacing the bench asked for.

### One spacing for all three, as ADR 0323 had

Keeps placement shared, which Auto no longer relies on since it places by its own numbers
(ADR 0424). It is what left Medium and Heavy ten times over.

### The option that won, and what it costs

The presets no longer share one placement: switching between them moves contacts as well as
thickens them, and the spacing was set on the bench, not on a print.

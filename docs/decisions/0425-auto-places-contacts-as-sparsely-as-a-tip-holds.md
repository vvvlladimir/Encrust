# 0425. Place Auto's contacts 3.3 mm apart, past 45 degrees, on 1.0 mm pillars

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

ADR 0323 placed every shipped preset's contacts 1.8 mm apart and held faces leaning past 40
degrees. The spacing was not measured: it was made denser so that a tip that does not bite
into the part would hold. Auto took the same numbers, and ADR 0324's 3.3 mm under steep faces.

A figure printed with the profile of an earlier version held on 330 contacts of 0.2 mm,
about 5.2 mm apart, past 55 degrees, with 5.8 ml of supports. Auto put 1904 contacts and
24.3 ml under the same figure. Its weakest tip had 8.3 times the pull it carries, against 1.4
for the profile that printed. A bite only changes the contact's area, which the bench
already measures. The open point generator for resin assumes one point per 11 mm² of new
area, which is about 3.3 mm apart.

## Decision

- **Auto's placement:** contacts 3.3 mm apart under a face looking straight down, 5.5 mm
  under one leaning just past the overhang angle, and faces held past 45 degrees.
- **Its four shapes stand on 1.0 mm pillars** instead of 1.2 mm.
- **A step's reach grows from 1.5, 2.4 and 3.2 mm to 2.7, 4.0 and 5.3 mm**, so that the
  faces just above a step stay held as part of it between the sparser contacts.
- Light, Medium and Heavy keep ADR 0323's numbers.

## Consequences

Across the bench's thirteen models, Auto takes 38.2 ml instead of 98.7, 3525 contacts
instead of 9947, and 279 mm² of scar instead of 741. The same nine models hold, and no tip
has less than 1.5 times the pull it carries. The figure above takes 11.3 ml.

The bench's area rule (120 mm² of flat plate per mm² of tip section, from one coupon print)
now calls every model overloaded, while the profile that printed the figure carried several
times that. A print of the figure on the new Auto, or a plate torn at 3.3 mm, reopens the
spacing. The area rule should be calibrated again against the print that held.

## Alternatives considered

### Keep 1.8 mm and lighten the shapes

Each tree would be cheaper, but the figure would still need six times the contacts of the
profile that printed it.

### Go back to the earlier version's 5.2 mm and 55 degrees

The earlier version's profile is one print of one figure, and its contacts bit 0.2 mm into
the part. 3.3 mm keeps twice its contacts until a print sets the spacing.

### The option that won, and what it costs

Each contact carries about three times the area it did. The weakest margin on the bench
fell from ×2.0 to ×1.6. A flat plate held only by the area rule now relies on strengthening
(ADR 0422) to add tips where the pull asks for them.

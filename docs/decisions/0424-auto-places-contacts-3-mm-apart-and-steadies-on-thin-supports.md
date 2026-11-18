# 0424. Place Auto's contacts 3.0 mm apart, past 45 degrees, on 1.0 mm pillars, and steady a part on thin supports

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

ADR 0323 placed every shipped preset's contacts 1.8 mm apart and held faces leaning past 40
degrees. The spacing was not measured: it was made denser so that a tip that does not bite
into the part would hold. Auto took the same numbers, and ADR 0324's 3.3 mm under steep faces.

A figure printed with the profile of an earlier version held on 330 contacts of 0.2 mm,
about 5.2 mm apart, past 55 degrees, with 5.8 ml of supports. Auto put 1904 contacts and
24.3 ml under the same figure. Its weakest tip had 8.3 times the pull it carries, against 1.4
for the profile that printed. The open point generator for resin assumes one point per 11 mm²
of new area, about 3.3 mm apart.

The calibration coupon (docs/design/analysis.md, "Calibration") limits how much flat
underside one tip holds: its plates held at 1.2 times the area rule and swayed at 1.5. The
bench called a tip overloaded past 1.0, below what the coupon held.

The supports that steady a part on a gentle slope (ADR 0326) carry nothing hanging; they keep
it from rocking on the peel. They were built like Auto Light, 0.30 mm tip on a 1.0 mm pillar.

## Decision

- **Auto's placement:** contacts 3.0 mm apart under a face looking straight down, 5.5 mm
  under one leaning just past the overhang angle, and faces held past 45 degrees.
- **Its shapes stand on 1.0 mm pillars** instead of 1.2 mm.
- **A step's reach grows from 1.5, 2.4 and 3.2 mm to 2.7, 4.0 and 5.3 mm**, so that the
  faces just above a step stay held as part of it between the sparser contacts.
- **Auto Stabilization is thinner:** a 0.27 mm tip and small pillar, a 0.7 mm pillar, trunks
  up to 1.0 mm, a 2.5 mm flare on a 4.0 mm pad, and 0.5 mm braces.
- **The bench calls a tip overloaded past 1.2 of the area rule**, the most the coupon's
  plates held. Placement and strengthening are not changed by this.
- Light, Medium and Heavy keep ADR 0323's numbers.

## Consequences

Across the bench's thirteen models, Auto takes 40.9 ml instead of 98.7. No tip carries more
than 1.12 of the area rule at its upper tenth, and the weakest pull margin is ×1.9.
One figure takes 340 medium and 47 heavy tips, some of them held by steps.

The stabilizer's tip is 0.27 mm, between the 0.24 mm that did not form on the stage-one print
and the 0.30 mm that did; it is not yet printed. The bench, set to 0.30, reads every such tip
as too thin, so four models it read as holding now fail on those tips alone. A stabilizer
tip that does not form, a part seen rocking where one stood, a print of the figure on the new
Auto, or a plate torn at 3.0 mm reopens these numbers.

A thinner stabilizer runs into thicker trunks where it used to merge with them; that exposed
a crash in growing trees, when a merge was judged beside one onto a thicker trunk, fixed with
this change.

## Alternatives considered

### Keep 1.8 mm and lighten the shapes

Each tree would be cheaper, but the figure would still need six times the contacts of the
profile that printed it.

### 3.3 mm under a level face

38.2 ml against 40.9, but the wide flat ceilings of the bench reached 1.3 to 1.36 of the area
rule, between what the coupon's plates held and where they swayed.

### Go back to the earlier version's 5.2 mm and 55 degrees

The earlier version's profile is one print of one figure, and its contacts bit 0.2 mm into
the part. 3.0 mm keeps about three times its contacts until a print sets the spacing.

### Keep the stabilizer's 0.30 mm tip

The thinnest the stage-one print formed. Rejected because a stabilizer holds nothing up, and a
smaller mark matters more on it than a tip that may not form.

### The option that won, and what it costs

Each contact carries about three times the area it did, and a flat plate held only by the
area rule relies on strengthening (ADR 0422) to add tips where the pull asks for them. The
stabilizer's tip is below the thinnest the stage-one print formed.

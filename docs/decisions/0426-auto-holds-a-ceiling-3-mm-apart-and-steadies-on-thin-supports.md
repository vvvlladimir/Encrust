# 0426. Hold Auto's ceilings 3.0 mm apart, and steady a part on thin supports

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

ADR 0425 spaced Auto's contacts 3.3 mm apart under a face looking straight down. Each of
them then carries more flat underside than before. The calibration coupon (docs/design/analysis.md,
"Calibration") gives the limit: its plates held at 1.2 times the area rule and swayed at 1.5.
At 3.3 mm the wide flat ceilings of the bench reached 1.3 to 1.36, between the two.

The bench called a tip overloaded past 1.0, below what the coupon held, so no model had
enough supports at any spacing that kept resin down.

The supports that steady a part on a gentle slope (ADR 0326) carry nothing hanging. They keep
it from rocking on the peel. They were built like Auto Light, with its 0.30 mm tip and 1.0 mm
pillar.

## Decision

- **Under a face looking straight down, Auto's contacts are 3.0 mm apart.** The 5.5 mm under a
  steep face and the 45 degrees of ADR 0425 stay.
- **The bench calls a tip overloaded past 1.2 of the area rule**, the most the coupon's
  plates held. Placement and strengthening are not changed by this.
- **Auto Stabilization is thinner:** a 0.27 mm tip and small pillar, a 0.7 mm pillar, trunks
  up to 1.0 mm, a 2.5 mm flare on a 4.0 mm pad, and 0.5 mm braces.

## Consequences

Across the bench's thirteen models, Auto takes 40.9 ml against 38.2 at 3.3 mm. No tip carries
more than 1.12 of the area rule at its upper tenth, and the weakest pull margin rises from
×1.6 to ×1.9. Sizing by share (ADR 0424) costs 40.9 ml against 39.5 with every tip lightest,
and 313 mm² of scar against 289. One figure takes 340 medium and 47 heavy tips, some of them
held by steps.

The tip is 0.27 mm, between the 0.24 mm that did not form on the stage-one print and the
0.30 mm that did; it is not yet printed. The bench, set to 0.30, reads every such tip as too
thin, so four models it read as holding at 0.30, nine of thirteen then, now fail on those tips
alone; volume is unchanged at 40.9 ml. A stabilizer tip that does not form, or a part seen
rocking where one stood, reopens the tip.

A thinner stabilizer now runs into thicker trunks where it used to merge with them. That
exposed a crash in growing trees, when a merge was judged beside one onto a thicker trunk. The
crash is fixed with this change.

## Alternatives considered

### Keep 3.3 mm

Less resin, but flat ceilings carry what the coupon's plates swayed at.

### Keep the stabilizer's 0.30 mm tip

The thinnest the stage-one print formed. Rejected because a stabilizer holds nothing up, and a
smaller mark matters more on it than a tip that may not form.

### The option that won, and what it costs

7 % more resin than ADR 0425, and a stabilizer tip below the thinnest the stage-one print
formed. Its bench verdicts now fail for that reason alone.

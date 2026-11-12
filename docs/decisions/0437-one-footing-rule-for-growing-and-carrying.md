# 0437. Growing and carrying stand a foot by one rule, and a refusal says why

- **Status:** Proposed
- **Date:** 2026-11-12

## Context

On a model grown with Lift off the plate at 0, about half the supports stand on the model, not
the plate. A carried foot or trunk looked for its footing by the rule growing tries first
(a face no steeper than 45 degrees), while growing falls back to 75 degrees when nothing else
is left. On a rough surface a step of 1 mm met a steeper face and the support turned red:
65% of the refusals of a dragged foot or trunk. A joint carried aside could give way only
when the trunk stood on the plate, so a trunk on the model had nothing to yield (ADR 0436).
Seven of 29 freshly grown supports did not pass the check a hand edit applies. And a red ghost
did not say which check refused it.

## Decision

- **One footing rule.** `carried` and `tip_carried` look for a footing as growing does: 45
  degrees, then 75 as a last resort. Nothing under the trunk takes it: eight directions at 1
  and 2 mm around it are tried.
- **A foot keeps to what it last stood on.** Each step of a drag starts from where the drag
  began, so the window hands `carried_held` what the previous step moved. While the plate or
  the model the foot last stood on is in reach, straight under or within 2 mm, the foot stays
  on it, at the place nearest where it was. Across an edge it switches once, 2 mm past it,
  rather than back and forth every frame.
- **A foot on the model slides.** The column carried across the plate stays whole and its
  trunk upright, and takes the footing found under it there. A column whose root is the neck
  a head hangs from is carried as one on the plate is, on a knee its head leans down to at 45
  degrees.
- **A root on the model may be lowered.** It joins the joints that give way, down only, over
  the foot it has; the pillar may be no shorter than the shortest one over that foot.
- **A host is eased before what stands on it is seated.** `carried_held` carries the held
  trees, eases each, then seats the supports standing on them on the eased trees: seated on
  the tree before its joints gave way, such a support hung in the air.
- **A refusal has a reason.** `allowed`, `eased` and `tip_carried` return `Refused`:
  `Collides`, `Leans`, `NoFooting` or `NoHold`, a tip carried onto a face looking up being
  `NoHold`. The window writes it beside the cursor while the carry is refused.

## Consequences

Fewer red drags on rough surfaces, and a hand sees what to change. A support may stand where
growing would have chosen to, on a face up to 75 degrees. The checks keep their cost: one
extra ring of at most 16 landings, each tried at two slopes, per carried column per frame, and
only where straight under fails or is not what the foot last stood on. Nothing about what counts as
printable changes, so no soft limit is added here.

## Alternatives considered

### A yellow warning for a lean to 55 degrees

It would let a support stand where the print may fail. It changes what is allowed, so it is
a decision of its own.

### Slide the foot along the surface by a search

It follows the model better, but each frame is a search along the surface. The two rings are
bounded, and a miss is refused with its reason.

### The rule growing and carrying share, and what it costs

A support carried onto a steep face stands there as grown ones may. The reason is one word,
not a picture of the place, so a refusal for two causes at once names the first found.

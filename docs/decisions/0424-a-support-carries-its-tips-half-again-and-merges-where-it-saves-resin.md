# 0424. Size every strut and trunk by the tips it carries, and merge where it saves resin

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

Since ADR 0423 Auto's tips are 0.30 mm wherever their load allows, yet the supports under them
were as heavy as before. The bench now splits the resin by part (`resin_by_part`): over
thirteen models branches took 29 to 46 % and feet 20 to 38 %, trunks 7 to 34 %. Every strut was
built to the pillar's 1.2 mm whatever it carried, one 0.30 mm tip included, and the pull on a
tip, 0.1 to 0.6 N, puts under 1 MPa in such a strut against 13 MPa a tip tears at
(ADR 0417). A trunk widened by its tip count through `trunk_exponent`; merging stopped at
`max_tips_per_trunk` and `max_strut_length_mm` (ADR 0338), fixed numbers that knew nothing of
how thick the tips were. A tree stood on 2.3 to 4.9 tips, each on a pad of its own.

## Decision

- **A tip is the weakest link.** Every strut, joint and trunk carrying `n` tips of contact
  radius `r_c` is built to `r_c √(1.5 n)`, half again its tips' section (`CARRYING_MARGIN`),
  so a tip tears at the part before anything under it breaks.
- **Slenderness in its own measure.** A trunk stands no taller than 15 widths (ADR 0242); a
  leaning strut runs no longer than 5 widths (`STRUT_SLENDERNESS`), the 6 mm at 1.2 mm that
  ended the swaying fans of ADR 0338.
- **Two bounds.** The pillar's diameter is the least a strut or trunk is built, the trunk cap
  the most; `SupportProfile::carrying_radius_mm` and `strut_radius_mm` hold all of it. Auto's
  shapes ship a 0.6 mm pillar, under rods of 0.5 mm that came through the stage-3 print.
- **The cap bounds a trunk.** A trunk takes tips while their section fits under the cap, 18 of
  Auto's 0.30 mm tips; a strut that would need more than the cap is refused.
  `trunk_exponent`, `max_tips_per_trunk` and `max_strut_length_mm` leave `[branching]`, with
  their two rows in the window; a file that has them still reads.
- **A merge must save resin.** Before a pair is offered, `branch/cost.rs` prices the two
  trunks and feet it leaves out against the struts and the shared trunk and foot, as cones,
  cylinders and the pad's disc; a merge that saves nothing is not made.

## Consequences

Over the bench's thirteen models with Auto the supports take 42.0 ml instead of 96.5, 48 to
70 % less on each; trees fall by up to 60 % where tips stand high enough to merge, and not at
all under slabs a few millimetres off the plate. No tip of any model keeps under ×1.5, no
contact more is left unheld, no trunk is slender, and fewer feet stand on the model. Feet are
now the larger part, up to 72 % on the low models: a pad sized by what it carries is the next
lever. Manual presets keep their pillars, so they change only where a trunk carries more than
its pillar holds.

Reopen when a 0.6 mm strut or a fan of a dozen tips sways or breaks in a print, or with a
second resin.

## Alternatives considered

### Size by the pull each tip carries

Each strut by the load the stack puts through it (ADR 0422): finer, but every edit in the
window regrows trees, and a regrow would then need the stack cured again.

### Keep the counts and lengths, thin the pillar

Fewer changes, but the limits stay numbers with no reason behind them, blind to a tip's width.

### The option that won, and what it costs

The rule is a ratio, not a measurement of the struts: no print has yet broken a 0.6 mm branch
or a dozen-tip fan on purpose. The cost of a merge is an estimate of cones that ignores
braces and the model in the way, so it can keep a merge a mesh would show is no saving.

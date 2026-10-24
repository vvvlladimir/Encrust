# 0347. Keep a tree near its tips: merge onto a lower front, drop past the part, and hold a strut's length and a trunk's tips

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

Merging (ADR 0040) offers each pair of fronts one meeting point, the highest their branch
angle allows, and refused a pair whose meeting point falls on one of them: a tip 3 mm
beside another and 3 mm higher never joined it, and joined a front at its own height
further off instead. Under a crowded part a neck stands half a millimetre from the model
and the strut to that point runs through a limb, so the pair stood as two straight
columns a millimetre apart. Where nothing was in the way merging never stopped: trunks of
17 to 22 tips rooted under 4 mm over the plate, fanned out on struts at 45 degrees that
users saw sway. A merged tree whose trunk failed the drawn check (ADR 0341) was stood tip
by tip. A blocked front leant at a third of the merge distance at least (ADR 0078), and a
headless contact (ADR 0331) tried those leans and a 25 mm search for ground (ADR 0342)
before the small pillar it was left headless for. An established SLA generator holds a
leaning bridge to 10 mm and a pillar to three bridges.

## Decision

- A meeting point on one front is a merge: only the other leans, onto that neck or joint.
  Only a contact is never met so.
- A pair refused for its struts or its landing is offered again with both fronts dropped
  `DROP_STEP_MM` = 1 mm straight down first, then 2 mm, until the meeting point reaches
  the plate; the drop is swept like a trunk under a neck.
- `[branching]` gains `max_strut_length_mm`, the longest strut of a merge, and
  `max_tips_per_trunk`, the most tips two merging fronts carry together, read from a file
  that lacks them as 6 and 6, shipped so, and edited in the Shape mode.
- A merged tree whose trunk or inner strut fails is split at its lowest joint and each
  half grown again, before three rounds end in tip by tip.
- A blocked front leans a millimetre further at each try, shortest first; a headless
  contact is run onto the model under it before any lean.

This supersedes standing such a tree tip by tip in ADR 0341, and the three reaches of ADR
0078.

## Consequences

On two saved figures, 47 trees with 41 lone columns became 25 with 3 alone on the plate,
and a 15 mm lean of a headless contact and a 9 mm strut across a brace are gone; no strut
of a merge runs past 6 mm. The bench figure stands the same tips; Light's supports take
0.65 ml against 0.56 before this, the automatic run's 0.92 against 0.73, on more and
lower trunks. A refused pair costs up to one sweep per millimetre of its height. Reopen if
prints of the shipped presets are seen to need fewer, heavier trees.

## Alternatives considered

### Take the nearest pair first instead of the highest meeting point

Tried: fewer lone columns on one figure and more long struts on the other, and a thinner
group then met a thicker one's trunk below its branch.

### A strut limit of 10 mm, as the established generator ships

On both figures it kept twice the struts over 5 mm that 6 mm does, and a tip passing a
full trunk for one further off.

### The option that won, and what it costs

Two more numbers in every profile and the window, more trunks and more resin, and a merge
search that can sweep a pair dozens of times where the part blocks it all the way down.

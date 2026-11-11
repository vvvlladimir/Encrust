# 0338. Keep a tree near its tips, and search far for ground before a front is given up

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

Merging (ADR 0040) offers each pair of fronts one meeting point, the highest their branch
angle allows, and refused a pair whose meeting point falls on one of them: a tip 3 mm beside
another and 3 mm higher never joined it, and joined a front at its own height further off
instead. Under a crowded part a neck stands half a millimetre from the model and the strut
to that point runs through a limb, so the pair stood as two straight columns a millimetre
apart. Where nothing was in the way merging never stopped: trunks of 17 to 22 tips rooted
under 4 mm over the plate, fanned out on struts at 45 degrees that users saw sway. And a
front with nothing under it leant for room at three reaches out to the merge distance
(ADR 0078); past that it was run onto the model or left unstood, and runs reported dozens of
supports with no room under a wide part that had open plate a little further off. An
established SLA generator holds a leaning bridge to 10 mm and a pillar to three bridges, and
searches for ground with a strut of about 25 mm at 45 degrees before giving up.

## Decision

- **A meeting point on one front is a merge**: only the other leans, onto that neck or
  joint. Only a contact is never met so.
- **A refused pair drops.** A pair refused for its struts or its landing is offered again
  with both fronts dropped `DROP_STEP_MM` = 1 mm straight down first, then 2 mm, until the
  meeting point reaches the plate; the drop is swept like a trunk under a neck.
- **A tree is held to its reach.** `[branching]` gains `max_strut_length_mm`, the longest
  strut of a merge, and `max_tips_per_trunk`, the most tips two merging fronts carry
  together, both read as 6 from a file that lacks them and edited in the Shape mode.
- **A blocked front leans a millimetre further at each try**, eight ways round, shortest
  first, no further than the merge distance or the height under it allows, onto a face up
  to 45 degrees and then the same onto one up to 75.
- **Then it searches for ground.** `search_for_ground` goes on from the merge distance out
  to where a strut `GROUND_SEARCH_MM` = 25 mm long at the profile's lean ends — 17.7 mm
  across at 45 degrees — every 1.5 mm across and 16 ways round, onto each face in turn, and
  takes the nearest knee whose strut is clear and under which a trunk lands. Merges keep the
  short lean: the search runs only for a front about to be given up.

This supersedes the three reaches of ADR 0078.

## Consequences

On two saved figures, 47 trees with 41 lone columns became 25 with 3 alone on the plate, and
no strut of a merge runs past its limit; a support under a wide part leans to the plate
beside it instead of being run onto the part or left out. Light's supports on the bench
figure take 0.65 ml against 0.56, the automatic run's 0.92 against 0.73, on more and lower
trunks; a long knee hangs more of a support at 45 degrees, and bracing reaches it as any
trunk. A refused pair costs up to one sweep per millimetre of its height, a blocked front up
to 192 more tries, each a beam and a landing: `--bench branch` grew the lifted 40 mm ball in
8.1 ms against 6.5, the one on the plate in 95 ms against 82, for the search alone. Reopen if
prints of the shipped presets need fewer, heavier trees.

## Alternatives considered

### Take the nearest pair first instead of the highest meeting point

Tried: fewer lone columns on one figure and more long struts on the other, and a thinner
group then met a thicker one's trunk below its branch.

### A strut limit of 10 mm everywhere, as the established generator ships

On both figures it kept twice the struts over 5 mm that 6 mm does, and a tip passing a full
trunk for one further off; only the automatic profile's thinnest shapes ship it (ADR 0325).

### Raise the merge distance, or a path of several knees

A wider merge would change the trees of every model to help a few points; a path round a
corner makes each bend a joint to check and a lever on the peel.

### The option that won, and what it costs

Two more numbers in every profile and the window, more trunks and more resin, a merge search
that can sweep a pair dozens of times where the part blocks it all the way down, and a
constant for the search no profile tunes, whose lean can carry a support further from what
it holds than any merge would.

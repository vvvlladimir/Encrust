# 0410. A single goal minimises its own reading alone

- **Status:** Proposed
- **Date:** 2026-11-07

## Context

Every preset but auto names one problem: the support, the pull, the stack, what shows.
Only the fewest supports ranked by that reading alone (ADR 0279). The least peel, the
shortest print and the clean surface each also counted suction cups as failing the print,
and islands and contacts with no room as hanging. The least peel and the clean surface
fell back on the fewest supports past their enough (ADR 0281, 0409). Each kept a band over
its best and a twentieth a turn had to win.

So a goal asked for by name could give up its own reading for a hazard the user had not
asked about. A model with a cup kept its pull high under the least peel, and a figure kept
a contact on its face for the resin under the clean surface. Auto weighs every hazard
already, and the user who wants them weighed asks for auto.

## Decision

- **A single-goal preset ranks by its own reading alone**, as the fewest supports did:
  past what fails every goal, the least reading wins outright, ties go to the tie-break,
  and any turn that lowers it is worth making. `Policy::alone` is that policy for every
  goal.
- **What fails every goal** stays: a pose past the build volume, or set on the plate with
  no face flat on it. The model cannot be printed that way at all.
- **Cups, tearing pulls, islands and contacts with no room** count for auto alone. No
  single goal reads them, and none has a band, an enough or a fallback.
- **Auto is unchanged.** `Policy::balanced` keeps each goal's band, enough and floor as
  auto weighs them; the least peel's 10 N enough is now auto's only.

## Consequences

Each preset does what its name says, and a hazard is the user's to settle: a drain hole
for a cup, more support or auto for an island. A slab that pulls 8 N flat is stood on its
edge by the least peel for less; a model is turned for any saving, however small.

The proof's noise no longer holds a single goal still: two poses a few hundredths apart
on a noisy reading are ordered by it. That is what the fewest supports already accepted.

`orient-lab`'s judge still holds every goal but the fewest supports to the soundest pose's
cups and islands, and the least peel and the clean surface to their enough. It now reports
those goals as failing what they no longer weigh, until the judge is changed to match.

Reopen if users of a single goal keep printing poses that fail, and reach for auto as a
workaround less often than they get bad prints.

## Alternatives considered

### Keep the hazards in every goal

That is what was there. It makes every goal half of auto, and the user who asks for the
least pull gets a heavier one whenever a cup is in the way.

### Keep enough and the fallback on the fewest supports

A pull under 10 N, or a contact on show, then turns the model to save resin. That ranks by
two problems, which is auto's work.

### The option that won, and what it costs

A single goal can now pick a pose with a sealed cup or a layer that tears off the film. The
preset's name is the only warning; Check finds the cup and the pull afterwards.

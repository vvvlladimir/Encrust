# 0410. A single goal minimises its own reading alone, and a clean surface is asked beside any preset

- **Status:** Proposed
- **Date:** 2026-11-07

## Context

Every preset but auto names one problem: the support, the pull, the stack. Only the fewest
supports ranked by that reading alone (ADR 0279). The least peel and the shortest print
also counted suction cups as failing the print, and islands and contacts with no room as
hanging. The least peel fell back on the fewest supports past its enough (ADR 0281). Each
kept a band over its best and a twentieth a turn had to win. So a goal asked for by name
could give up its own reading for a hazard the user had not asked about.

ADR 0409 made the clean surface a fifth preset. But keeping a face clean is not a goal
beside resin, pull and time: a person wants a figure's face clean and its resin least, or
its face clean and its print short. As a preset it could not be asked with any of them. It
read what shows off the shape alone; ADR 0409 left painting it by hand as step 2.

## Decision

- **A single-goal preset ranks by its own reading alone**, as the fewest supports did:
  past what fails every goal, the least reading wins outright, ties go to the tie-break,
  and any turn that lowers it is worth making (`Policy::alone`). What fails every goal
  stays: a pose past the build volume, or set on the plate with no face flat on it.
  Cups, tearing pulls, islands and contacts with no room count for auto alone. Auto is
  unchanged (`Policy::balanced`).
- **A clean surface is a switch beside any preset**, `OrientSettings::clean_surface`, and
  `OrientGoal::CleanSurface` goes. Asked, the rule keeps, after what fails and what hangs,
  the poses within a tenth and one contact of the fewest on show among the sound; the
  preset ranks those as it ranks all. The search weighs what shows at 1.0 beside the
  preset's own terms, and its shortlist keeps the two that show least.
- **A trade away from it is offered**, never chosen: a pose past the band that wins a
  quarter of the preset's reading over the chosen — for auto a quarter of the product of
  shares — and that no sound pose beats on both what shows and the preset, is ruled
  `Ruling::Traded`, offered, and ranked right behind the first. The rest are
  `Ruling::Shows`.
- **What shows can be painted.** A model's `shown_painted` region, saved in `.encrust`,
  is painted in Position beside the switch. Where any face is painted, the painted faces
  read 1 and the rest 0 (`Prominence::shown`, `Printed::shown`); otherwise what shows is
  read off the shape. The proof reads it on the outer surface the region numbers.
- **The command line** has `--clean-surface` and a recipe's orient step `clean_surface`.
  Painting is the window's, as the relief's is (ADR 0318).

## Consequences

Each preset does what its name says, and a hazard is the user's to settle: a drain hole
for a cup, support or auto for an island. A slab that pulls 8 N flat is stood on its edge
by the least peel; a model is turned for any saving. The proof's noise no longer holds a
single goal still, as the fewest supports already accepted.

A face can be kept clean with the least resin, the least pull or the shortest print. One
contact on show is the band's slack, so a ball-like head may stand with its face to the
side rather than up when that saves resin. A trade far better on the preset is never lost:
the user steps to it.

`orient-lab`'s judge still holds every goal but the fewest supports to the soundest pose's
cups and islands and the least peel to its enough, and no longer judges a clean surface. It
reports those goals as failing what they no longer weigh until it is changed to match. A
recipe written with the goal `clean-surface` no longer reads.

The tenth, the contact and the quarter are set by eye. Reopen if users keep stepping to
trades, which says the band is too tight, or find scars on painted faces, too loose.

## Alternatives considered

### Keep the hazards, the enough and the fallback in every goal

That is what was there. It makes every goal half of auto, and the user who asks for the
least pull gets a heavier one whenever a cup is in the way.

### Clean surface first, the preset only between equals

Then the switch is the old preset under another name: a tenth of a contact less on show
beats any saving.

### What shows as one more weighted term

A weight is a price in another unit; it is the mixing of goals auto alone does, and it
leaves no way to say a trade is worth offering.

### Painting adding to the shape's reading

Painted faces as 1 over the read prominence elsewhere keeps scars counted on surface the
user did not mark. Painting is the user saying where the face is; the rest then is not.

### The option that won, and what it costs

A single goal can pick a pose with a sealed cup or a tearing layer; the preset's name is
the only warning, and Check finds them afterwards. A clean surface with nothing painted is
only as good as the prominence, and with a band of one contact it can move a scar from the
face to its edge.

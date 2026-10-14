# 0278. Rank the search and the proof by one rule, and prove a pose with its supports

- **Status:** Accepted
- **Date:** 2026-10-15

## Context

ADR 0275 ranks the search's candidates by priority; ADR 0277 ranks what the proof
printed by a rule of its own. The two disagreed: the search counted a cup four, a tearing
pull two and an unseated pose one, and weighed a turn's gain on the total; the proof
counted cups and tears alike, took the strict best on the goal's reading and weighed a
turn on that. A change to one stage or one goal moved the other.

The proof also sketched each pose without its supports, so it read no islands and no
contacts with no room. On a figure it chose a pose upside down with 13 islands over one
tilted 15° with 3, and stood a lever bare on the plate with five overhangs unheld.
`orient-lab` measured both on 23 models and 8 bodies.

## Decision

- **One rule, `core_plate::rank`, ranks every stage.** A stage hands it `Readings` — what
  fails the print, islands and contacts left hanging where it reads them, the goal's own
  reading, a tie-break, and which is the way the model stands — and the noise of the
  goal's unit. In order: what fails the print, exactly; islands and hanging within a
  quarter and two of the fewest; the goal's reading within a tenth of the best and the
  noise; the tie-break. The search breaks ties on its total, the proof on the search's
  order. The noise is 0.002 of the search's dimensionless term, and two contacts, half a
  newton or five layers once printed.
- **A turn is weighed on the goal's own reading**, not the total: the model stays as it
  stands unless a turn fails less, standing is out of the band, or the turn wins 5 % of
  that reading. Of two ways of standing still, on supports and bare, the first in the
  search's order is the one a turn is weighed against.
- **The proof prints each pose with its supports,** grown and meshed, on its coarse
  stack, and ranks on the islands and hanging contacts that reads. The sketch without
  supports is gone, and `print_out` is the one way a pose is read.
- `core_plate::LOOKS_UP` is the one reading of a face that looks up in its file.

## Consequences

On `orient-lab`, with 0274–0277 as the baseline: the bodies pass 31 goals of 32 against
29, the corpus 64 of 92 against 62, and the fewest supports 16 of 23 against 14. A lever
and an island part no longer stand bare with overhangs unheld. Choosing takes a fifth
longer: 101 s against 83 s over the eleven largest models.

No goal turns a model for what another weighs: a slab lying on the plate stays flat for
the fewest supports, where it used to stand on its edge for the peel in the total.

A coarse stack reads about half the islands a print does — 7 for 13, 1 for 3 — in the same
order, so the band of a quarter and two is loose on it. A figure still turns upside down
when no upright pose reaches the finalists. Reopen this when the band is tuned on coarse
counts, or when the finalists are widened.

## Alternatives considered

### Keep two rules, each tuned to its stage

The stages read in different units, which seemed to call for different rules. Rejected:
the units differ only in their noise, which is one number a stage hands over, and two
rules let a change to one stage move the other unseen.

### Read islands off the sketch where a contact lands

Islands of the model cured alone, held where a contact stands under them, would cost
less than growing the supports. Rejected for now: it needs islands placed in plate
coordinates against contacts, and a print with supports reads them the way the print
does, for a fifth more time.

### The option that won, and what it costs

A fifth more time for every proof, and a band for islands set on a print's counts and
applied to a coarse stack's. A figure upside down with 12 islands still passes the band
against an upright pose with 4 when the coarse stack reads them as 7 and 4.

# 0275. Choose an orientation by priority, refuse cups and tearing pulls, and stay unless a turn is worth it

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

After ADR 0274 the search still picks the lowest weighted total. A goal could win its own
reading by printing a risk: Shortest print laid a peel slab flat at 144 N against the
30 N `core-analysis` names, and Clean top kept a box open side up, so its floor sealed a
suction cup. Nothing kept a model standing nearly right from being turned for a fraction
of a percent. The search also looked coarse: a 14° sphere, and only 8 candidates cut.

## Decision

- The second pass finds suction cups on the sections it already cuts, by the rule
  `core-analysis` uses on the cured stack. A hole is sealed when the point inside it
  stands, one section down, on material or on a sealed hole. The plate seals the first
  section, and supports do not. No raster and no new dependency: `core-plate` keeps the
  graph it has.
- `OrientSettings` carries `peel_limit_mm4`, 30 N over `core-analysis`'s N per mm⁴, and
  `min_cup_mm2`, its 25 mm². A test in `encrust-cli`, which sees both crates, pins them
  equal.
- The cut candidates, and always the way the model stands now, are ranked by priority.
  First comes what fails the print: a cup counts two, a pull past the limit one. Then the
  goal's own term, unweighted, within a tenth of the best. Then the total.
- The model stays as it stands unless the turn fails less, brings the goal's own term
  back within the band, or wins 5 % of the total.
- A tip costs three flat patches one contact holds rather than one, superseding that
  clause of 0274.

## Consequences

On `orient-lab`'s 16 bodies, against 0274:

- Shortest print takes 120 layers on the 40 mm slab, against 299, and 70 on the relief
  tile, against 142.
- The tilted mushroom's four islands are gone, and the open box no longer cups under
  Clean top.
- The peel slab's 144 N, under 0274 and now, is the raft under its 1225 contacts. Tilted
  6°, the model's own sections are under the cap, and the cap first bites on a slab
  that would lie flat, as the 100 mm test slab does.

What fails now comes from support placement. A tip gets no contact (TODO-13), a sphere
gets islands at any turn (TODO-14), a slab on its side gets contacts with no room
(TODO-15), and a raft ring seals a cup against the plate.

The 100k-face ball orients in 33.8 ms, against 32.0 ms.

## Alternatives considered

### Search wider and walk the winner downhill

16 candidates cut by total and 4 more by the goal's own term, then a pattern search
around the winner at 7°, 3.5° and 1.75°. It
was measured, and dropped. Failures across the four goals went from 60 to 110, and the
ball took 80 ms. The better the search found low scores, the more of what it found
leaned on what the score cannot see: poses on a corner, with every face just past the
overhang angle, that the support generator leaves on an unheld tip or an island. The
signal to reopen is the support fixes above landing.

### Check cups on the raster in the caller

Exact, the same reading as the report, but it needs the panel and a raster per
candidate. It would also put the check in the window and the command line twice.
Contours read the same rule at the cost of missing a vent between two sections.

### Priorities, and what they cost

The band and the 5 % are set by eye on the lab, like the weights. A goal's own term
decides even where the difference between two poses does not matter to a print. The
bracket's 0.3 N against 0.2 N is the one the lab still shows.

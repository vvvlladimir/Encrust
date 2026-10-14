# 0277. Prove an orientation by printing its finalists, and search the poses a person tries

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

On five real models `orient-lab` showed the search of ADR 0275 losing to a pose by hand
more often than not, for reasons its score cannot see:

- it ranks on a prediction. Clean top left contacts on up-facing faces it had scored as
  clean, and poses were chosen with a suction cup the 24 sections had not met;
- a leaning model's shadow overstates its sections, so the tilt a person reaches for
  first never reached the second pass: Least peel took 69 N where 35° of lean gives 6 N;
- every model was judged on supports, so a test piece drawn to print on the plate was
  turned onto its side;
- on the plate, a pose balanced on an edge scored as held by it.

## Decision

- **The poses a person tries are always cut:** the model leaned 15°, 30° and 45° from the
  way it stands, eight ways round, and the two best candidates on each reading alone —
  overhang, shadow, height, top — beside the best eight by total.
- **A model that needs no supports rests on the plate.** With `rest_on_plate`, the way
  the model stands and its six largest flat faces are scored set on the plate. One with a
  face flat on it, under a hundredth of its surface hanging, no cup and no tearing pull is
  taken ahead of every supported pose, and `Oriented::on_plate` says so. One hanging more
  but with a quarter of its shadow flat on the plate comes last among the finalists,
  for a caller able to print it bare and see.
- **Set on the plate with no face flat on it is a failure**, under a cup and a tearing
  pull.
- **`core-plate` answers finalists**, best first and at least 10° apart, the way the model
  stands always among them, and `orient` is the first.
- **`core-engine` proves them.** `orient_proven` stands the first four and the way the
  model stands on supports, sets every pose that may rest on the plate there bare, cuts
  and cures each, and reads the result as a finished plate is read. A bare pose leaving
  more than sixteen places hanging is dropped. It keeps the poses
  with the fewest suction cups and tearing pulls; of those, the ones within a quarter and
  two of the fewest failures, an island or a contact with no room counting one and a cup
  or a tear ten; and of those the best on the goal's own reading: contacts, newtons,
  layers, contacts on up-facing faces. The model stays as it stands unless a turn wins
  5 % of it.
- **The trials are coarse and run together:** the tallest pose in about 160 layers, a
  0.2 mm pixel, one trial a `rayon` task. At the print's own resolution five trials of a
  230 ml model took 35 s; coarse they take 2 s and choose the same poses.
- `stand_under` moves from `encrust-cli` into `core-engine`, which the proof needs and the
  command line now wraps. `core-engine` takes `core-plate` and `rayon` as dependencies.
- `orient-lab` prints through the same proof, adds the model as it stands on the plate to
  the poses by hand, and counts a goal as passing when it fails no more than the soundest
  of those and either reads within a tenth of the best or is rid of a cup or a tear
  they all have.

## Consequences

The window proves an orientation whenever a printer is chosen, and takes the search at
its word otherwise. A run costs the search and its slowest trial: about two seconds a
million faces, on the worker thread it already had. Clean top and the cup rule are now
judged by the contacts and the analysis a print gets rather than by a prediction of them.

The command line's `--orient` still takes the search's first answer, before it hollows.
It keeps a pose found resting on the plate there, and stands no supports under it.

Reopen this if a coarse trial is caught ranking two poses the other way round from the
print, or once supports hold every island, which would let the failure band go.

## Alternatives considered

### Forbid a pose that turns the model upside down, or weigh faces by how visible they are

It would keep supports off a figure's face under every goal. But which way is up is not
in a technical part at all, and Fewest supports promises nothing about where they land.
Clean top is the goal that reads the file's own up; a brush that marks faces to keep
clean can say the rest later.

### A finer cup reading in `core-plate`

More sections, or the analysis's own rule on a raster. It would still be a prediction
of what the proof measures, and `core-plate` would need the rasteriser.

### What the chosen option costs

The proof is only as good as the supports it stands. The generator still leaves islands
finer than its grid, so the failure band carries a slack for them, and a pose is chosen
among at most five: a better one the search never short-listed is not found.

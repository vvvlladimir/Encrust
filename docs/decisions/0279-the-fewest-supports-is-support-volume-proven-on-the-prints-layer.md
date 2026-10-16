# 0279. Choose the fewest supports by support volume, proven on the print's layer

- **Status:** Accepted; the lift a pose is judged at in the window is superseded by 0291
- **Date:** 2026-10-15

## Context

ADR 0278 proves an orientation by printing its finalists with supports, on a stack about
three times coarser than the print. `orient-lab` printed every finalist again at full
resolution: support placement reads the layer it is cut at, and the coarse stack counted
189 contacts on a figure leaned 45° where the print stands 442. The proof ranked on
numbers that were not the print's.

The fewest supports also counted contacts and refused any pose with a suction cup. A
user's figure stood as drawn wanted 3.8 ml of supports; the goal leaned it 45° onto 5.7 ml
of scaffolding to escape a cup a drain hole closes. What a person minimising supports
pays for is resin and scaffolding, and a cup is another goal's concern. Which way is up is
not known either, so nothing keeps a model from turning over.

## Decision

- **The fewest supports reads the resin in the supports**: `Printout::supports_ml`, the
  volume of the grown supports as the plate's summary counts it. The search predicts it as
  the plan area past the overhang angle times the height it is held at, lift included
  (`OrientSettings::lift_mm`).
- **Suction is not fatal to the fewest supports.** `Readings::suction` carries cups and
  tearing pulls apart from `fatal`; every other goal still weighs them as failing the
  print. Nothing about upside down is weighed.
- **A model can stay on the plate with supports only under what hangs over it**, its raft
  with them where the profile has one: `core_engine::stand_seated`. A pose the search
  sets on the plate is proven that way, no longer dropped for needing supports, and the
  window and the command line stand a model the way it was proven — on the plate, or
  lifted by the profile's own lift (`Turn::lift_mm`).
- **The fewest supports takes strictly the least support volume.** Past what fails every
  goal — a pose past the build volume, one balanced on the plate on a point — nothing else
  weighs: not islands (`Readings::unheld`), not contacts with no room for a column
  (`Readings::roomless`), no band over the least, and any turn that saves support is worth
  making. The window orients with the raft the model stands on (`SupportTool::built_for`).
- **A pose that overflows the build volume fails the print** for every goal
  (`Printout::overflows`), turned about the vertical however it fits.
- **A proof places supports on the print's own layer**; only the cure that reads cups,
  pulls and islands stays coarse.
- **For every other goal the band over islands and hanging is a quarter and one**; within
  the goal's band the fewest of them rank first, then the goal's reading to its noise, then
  the search's order. A turn has to win more than that noise as well as 5 %, and is weighed
  against the best ranked way of standing still.
- **The proof prints more finalists where they are cheap** — four at a million faces, up
  to sixteen for a light model under 200 mm — and the fewest supports spreads them 20°
  apart, not 10°.
- Speed: the slicer stitches a plane without a map of vectors, support placement hashes
  its buckets with `FastMap` and takes over the largest set where pieces join, and
  `Run::measuring` cures without drawing a thumbnail.

## Consequences

On `orient-lab`, judged on strictly the least support volume known, the fewest supports
passes 17 models of 23, 21 with a raft: where it fails, the least pose was found by another
goal and not among its finalists, or the run was over time; a statue goes from 20 ml to 3.6 ml, a figure stays on its base at 4.1 ml where
it was lifted onto 8.2, a cup stays on its floor. Every goal passes 74 of 92 against 65;
the bodies 32 of 32.

A ghost on its base needs 1.6 ml held where it hangs, 3.7 ml with a raft, against 11.4 ml
lifted: with a raft it now stays as it stands, without one it leans for 1.2 ml. A pose it
picks may cup, or leave contacts with no room: the user drains it, or picks another goal. It may also stand a
figure on its head. Still failing: two figures whose coarse islands miss what the print
leaves, a stand whose best pose is not among the finalists, a mask the same. Reopen when
islands can be read without a full cure, or when the search's volume prediction is
measured against the generator's on more models.

## Alternatives considered

### Keep contacts, refuse cups, keep figures upright

What 0278 did. Rejected: contacts and cups are not what this goal's user is minimising,
and the upright rule is a guess at a top the file does not state.

### Place supports on a layer twice the print's

Half the slicing. Rejected: a statue read 479 contacts where the print stands 405.

### Count the islands support placement leaves alone

Placement sees the specks it is too small to hold. Rejected: they track a print's islands
at 0.71 across models, and not in order within one.

### Keep the bands for the fewest supports

Islands within a quarter and one, the volume within a tenth, a turn worth 5 %. Rejected:
the user asked for the least support for the profile set up, raft included, and a band
let a lifted lean win over a model standing on its raft.

### The option that won, and what it costs

A proof slices every finalist at the print's layer, and more of them for a light model:
the proof takes 91 s over the corpus against 69 s. The goal will turn models over and keep
cups the old one avoided.

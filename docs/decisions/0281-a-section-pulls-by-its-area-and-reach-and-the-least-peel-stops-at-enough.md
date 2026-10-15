# 0281. A section pulls by its area and reach, and the least peel stops at enough

- **Status:** Accepted
- **Date:** 2026-10-15

## Context

A layer's pull was Stefan's squeeze film, the torsion constant `J ≈ A⁴ / (4π² I_p)` times a
fixed constant: a disc pulls as `R⁴`, a strip as its width cubed. Measured, the force
follows the area and only weakly the shape — a disc and a star of 314 mm² pull 6.16 N and
4.9 N (Pan et al., *Rapid Prototyping Journal* 23(2), 2017) — and a film peeling off its
edge, as a tilting vat does, never pulls a strip to nothing. With `R⁴` the least peel
read a 98 mm plate tilted 45° at 0.0 N and turned miniatures over for a tenth of a newton
below a noise of half one. `core-plate` also scored its own copy of the law, with the
limit and the smallest cup copied by hand from `core-analysis`.

The goal had no level at which it was satisfied, and its policy could only weigh its own
reading, so a pose pulling 2 N lost to one pulling 1 N whatever supports it cost.

## Decision

- **A piece pulls `A (PULL_N_PER_MM2 + PULL_N_PER_MM3 r)` newtons**, `r` its reach
  `sqrt(A³ / (8 π² I_p))`, which is `A / L` for a disc: the two constants' ratio is the line
  through Pan's disc and star, their scale a 100 cm² disc at 100 N as before.
  `core_analysis::pull_n` is the one function; pieces, layers, `Peak` and `Cut` carry
  newtons, and `equivalent_disc_mm` inverts it. `PEEL_LIMIT_N` stays 30 N.
- **`core-plate` depends on `core-analysis`** for `pull_n`, `disc_pull_n`,
  `equivalent_disc_mm`, `PEEL_LIMIT_N` and `MIN_CUP_MM2`; `OrientSettings::peel_limit_n`
  replaces `peel_limit_mm4`.
- **`Readings` carry every goal's reading** (`goals`, in the order of `OrientGoal::ALL`)
  in place of one `primary`, and `rank` and `rulings` take the stage's `Noise` for every
  goal in place of one slack. A `Policy` names its goal, a reading at or under which every
  candidate reads alike (`enough`), and the goal that decides between those (`fallback`);
  a turn between two that both have enough must spare 5 % of the fallback's reading and
  more than its noise.
- **The least peel has enough at a third of the limit, 10 N, and falls back on the
  fewest supports.** Its search reads the pull in newtons, as its proof does, to half a
  newton. The other goals have neither.
- The search holds a face within a degree of the overhang angle as hanging, since
  placement holds much of one that meets it exactly: a plate leaned 45° read almost no
  support and printed 1.66 ml, more than it on its edge.
- `orient-lab` judges the least peel with an `enough` of its own and fails a choice that
  has enough but takes more than a quarter and a strut over the least support among sound
  poses that have it.

## Consequences

A strip now pulls what its area does, so a turned plate reads newtons, not zero; a model
already under 10 N stays as it stands or turns only to save support, which the fewest
supports already does well. The search and the proof read one law and one limit, and a
check that copies of them agree is gone. A future goal that weighs several readings is a
policy over `goals`, with no stage changed.

The window's peel and lever warnings move: a mid-sized section pulls more than it did, a
large disc the same. The scale is known to about a factor of three, which is why enough is
a third of the limit; a measured force on a known printer reopens both numbers, and a
result on `orient-lab` that a pose under 10 N tore reopens enough.

## Alternatives considered

### Keep Stefan and add only a floor

The floor would stop the turns for a tenth of a newton, but a plate tilted 45° would still
read zero, so the goal would still pick by a number that is not the force.

### Pull proportional to area alone

Simplest and closer to the measurements than `R⁴`, but blind to shape: Pan's star pulls a
fifth less than the disc, and the reach term costs nothing to read off the moments.

### Trace the perimeter for `A / L`

Exact by definition, but a perimeter on pixels is a staircase, too long by up to 4/π on a
diagonal, and the contour cut and the cured layer would disagree; the reach from moments is
within a fifth and the same for both.

### The option that won, and what it costs

Two constants fitted on one set of measurements, made on PDMS pulled straight up, stand in
for a PFA film peeled by a tilt; and `core-plate` takes a dependency it did without.

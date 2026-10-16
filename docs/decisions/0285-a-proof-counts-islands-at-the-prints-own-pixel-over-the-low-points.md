# 0285. A proof counts islands at the print's own pixel and layer, over the low points

- **Status:** Accepted
- **Date:** 2026-10-15

## Context

The proof (ADR 0277, 0279) cures each finalist at a 0.2 mm pixel and up to 0.15 mm a
layer. The islands a print actually cures are specks the support generator leaves alone,
mostly under 0.1 mm², down to the 0.01 mm² below which `core-analysis` names none (about
20 pixels of a 19 × 24 µm panel). The coarse stack read none
of them: on V3 Puck it saw 0 islands where the print has 7 to 15, and it misjudged the
finalists of every figure in `orient-lab`. Curing at full resolution reads them, but costs
0.4 to 1.6 s a pose, four times the coarse cure.

A piece of a layer can only be born where the surface turns from going down to going up:
at a vertex lower than every vertex it shares a face with, or on a flat underside. On the
corpus, 36 of 43 islands of a full print sat within 0.15 mm and one layer of such a point,
and all within 1.1 mm.

## Decision

- **The proof counts islands with `trial::islands`, not with the coarse cure.** Each low
  point of the model gets a window 1.5 mm round it, a few layers high. The faces in the
  window are cut on the print's own planes. The contours that never leave the window are
  whole, so they are drawn on the print's own pixel grid, with the resin's offsets, and
  `core_analysis::islands` names what stands on nothing. Each piece found this way is then
  checked on the whole plate's contours at its layer and the one under it, drawn into the
  same window, so material reaching in from outside is not missed.
- A flat underside counts as one low point at its middle, and only if it is narrower than
  1.5 mm; a wider one is left to the supports.
- The coarse cure stays as it was, for cups, the pull and the stack.
- **Islands and hanging contacts within a quarter and two of the fewest read alike.** That
  was one, because the coarse stack undercounted islands.
- **A pose set on the plate with something hanging off it fails like a cup.** It fails
  every goal that weighs hazards, which is every goal except the fewest supports.
  `orient-lab` already judged it so (`seated`).
- `core_pipeline::Tolerance::applied` is public, so a window is offset as its layer is.

## Consequences

On the 254 finalists of `orient-lab --full-finalists auto`, islands and hanging contacts
agree with the full print exactly on 245, against an error of about four a pose before, and
the coarse stack's cups and tearing pulls still agree on 253. Counting costs 0.04 to 0.3 s
a pose, depending on how many low points the model has; the corpus chooses in the time it
took before, since cutting contours got faster in the same step.

A piece born away from every low point is missed. A model that is all fine tips, like a
calibration plate stood on its cones, pays for a thousand windows. A full print that
disagrees with the proof on islands, in `orient-lab`'s trace, is the signal to revisit.

## Alternatives considered

### A finer coarse cure

A 0.1 or 0.05 mm pixel, or the print's layer at 0.2 mm, still missed islands by 3 to 3.6
on average. Only the print's own pixel and layer together agree, at the cost of a full
print.

### The windows alone

The first pass reads only the contours that close inside a window. A piece that thins
upwards looks like a new piece at the layer where it first fits, so on its own that pass
counted five times too many.

### A join tree to drop low points that merge before the next plane

This is exact, and on the calibration plate it halves the windows. But it is a sort and a
pass over every face. Once the faces are filed into windows by a bitmap, it cost more than
it saved on every other model, so it was dropped.

### Windows over the low points, and what they cost

They count what the print counts only where a piece is born at a low point, and a low
point is a vertex. On a mesh too coarse to have one where a speck forms, the speck is not
counted.

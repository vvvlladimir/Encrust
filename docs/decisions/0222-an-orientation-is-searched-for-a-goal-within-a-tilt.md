# 0222. Search an orientation for a goal, within a tilt

- **Status:** Accepted; where the goal and the tilt are offered is superseded by 0236, the goals' weights by 0274
- **Date:** 2026-10-11

## Context

ADR 0087 ranks every candidate "down" by one weighted sum of overhang, peel, height and
footprint, with the weights fixed in `core-plate`. A user who wants the fewest layers, or a
face kept free of support marks, had no way to say so, and a model already standing close
to right could be turned upside down by a small difference in score. The window's Orient
section offers a goal and a tilt limit for both reasons (ADR 0218).

## Decision

`OrientSettings` carries a `goal: OrientGoal` — `FewestSupports`, `LeastPeel`,
`ShortestPrint`, `CleanTop` — and `max_tilt_deg: Option<Scalar>` measured from
`down_now`, the way down the model stands, in its own space.

- A goal is a set of weights over the same terms. `FewestSupports` keeps the weights 0087
  tuned, so the default search is unchanged; `LeastPeel` triples the peel and halves the
  overhang; `ShortestPrint` multiplies the height by six and halves the overhang.
- `CleanTop` adds one term, `Score::top_overhang_mm2`: the overhang on faces whose normal
  in the file is within 60° of +Z, weighted three times on top of the overhang it already
  is. The file's +Z is the only statement of "the side meant to be seen" a mesh carries.
- A tilt limit drops every candidate further than it from `down_now`, and always keeps
  `down_now` itself, so a limit of zero leaves the model standing as it is.

## Consequences

The goals are four presets of the weights, not a weight editor: they are as untrained as
0087's weights are, and a goal that loses to the default on the fixtures is a reason to
retune it. `CleanTop` means nothing for a file authored lying on its side; that is the
signal to let the user name the side instead.

## Alternatives considered

### Faces painted as "keep off the plate"

The mockup's "Respect blockers". No such marking exists, it would be new per-model data in
the project, and the file's +Z covers the common case. Dropped.

### Weights exposed as sliders

Four numbers nobody can predict the effect of. A named goal says what it is for.

### Goals as presets, and what it costs

A goal cannot be mixed with another, and a print that wants both fewer supports and fewer
layers takes whichever weighs more.

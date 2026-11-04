# 0396. A blocker reaches past the wall, can fit the model, and runs on as a rod

- **Status:** Proposed
- **Date:** 2026-11-03

## Context

A click in the Hollow tool drops a ball blocker centred on the surface, with the
inspector's size as its radius. The cavity starts a wall's thickness under the surface, so
any ball no wider than the wall lies inside material that is solid anyway: on the default
2 mm wall, sizes from 0.5 to 2 mm changed nothing, and users read the size field as broken.
Keeping a limb solid also took a guess at its width and a row of balls along it, though
`Blocker` was always a rod with round ends (ADR 0076).

## Decision

- `ModelHollow::asking` walls every blocker a user placed by the shell's thickness, as
  `sleeves` already does for channels and walled holes. A blocker's radius is how far it
  reaches past the wall into the cavity; the stored blocker is unchanged.
- `fitted_blocker` fits a ball to the model under a click: a ray from the click straight in
  to the far side, the ball centred halfway with half that width. Where the ray finds no far
  side, the size the panel asks for stands.
- `ModelHollow::rod_to` runs a rod from the last blocker's end to the new ball, as wide as
  it; in the window it is a shift-click, as a line of supports is (ADR 0330).
- `BlockerSize` holds the choice, fitted unless a user picks a size; where it is kept is
  ADR 0397.
- `blocker_under` makes that choice for the window and for `--keep-solid` alike, the
  command line's points standing for clicks and the points run together for shift-clicks.

The geometry is in `docs/design/hollowing.md`.

## Consequences

A blocker of any size changes the cavity, and the size means the same on every wall. A
project saved before keeps its blockers but they reach a wall further than they did, so its
cavity is a little smaller when hollowed again. A fitted ball measures the model along the
click's normal alone: on a slanted face or a cut's cap it takes the width that way, which
may be wider than the part across. A fitted ball on a thick body is as large as the body;
the ghost shows it before the click.

## Alternatives considered

### Centre the ball a wall's depth under the click

Gives the same reach, but the marker would stand inside the model, out of sight, and a wall
changed after the click would leave it short or deep.

### Fit the rod to a skeleton of the model

Finds a limb's axis without clicks, but a medial axis of an arbitrary mesh is a project of
its own and noisy on scans; a click at each end serves the same need now.

### Fit by the panel's size, walled when placed

Stores the wall on the blocker, which goes stale when the wall changes. Walling at the run,
from the wall asked, keeps one source for the thickness.

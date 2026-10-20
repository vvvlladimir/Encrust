# 0320. State an infill by its cell and its density, and derive its walls and holes

- **Status:** Accepted
- **Date:** 2026-10-20

## Context

ADR 0302 had a hive or grid state five sizes: the cell, the wall, the crossing holes'
diameter, the height of their first row and the spacing between rows, while a scaffold took
a cell and a density. The Infill block then showed five rows for one pattern and two for
another. A user rarely knows what wall a cavity needs, and what they want to set is how much
resin the lattice takes and how coarse it is. A hole too wide or rows too close were refused
on every change, so most of the rows were ways to hit an error.

## Decision

`InfillSettings` is `pattern`, `cell_mm` and `density`, for all three patterns. A hive's or
grid's wall is what leaves `density` of a plane of cells solid, `t = a (1/sqrt(1 - d) - 1)`
for an opening `a` across, and a scaffold's strut is inverted from `3x² - 2x³` as before.
Crossing holes are two fifths of the cell across, the first row three fifths of a cell over
the floor and the next every 1.25 cells: a hole stays under a hexagon's side, and rows stay
apart, at every size, so `BadInfillWall` and `BadHoles` are gone. The window shows the wall
the two numbers make under them and warns under 0.5 mm. The command line drops
`--infill-wall` and `--infill-hole*`, and `--infill-density` sizes every pattern.

A project saved with a `wall_mm` of its own reads it as the density it made, so its walls
come back as they were; its holes follow the new rule.

## Consequences

The block is the pattern, two numbers and one line for every pattern, and nothing a user
types can be refused but a cell of no size or a density outside 0 to 1. The density is that
of the walls under the holes; the holes take a little more out. Walls and holes can no longer
be sized apart: a thick-walled lattice with small cells takes a high density, and a hole
size the rule does not give cannot be had. Reopen this if a print shows the holes too small
to drain or too large to hold.

## Alternatives considered

### Keep every size, behind a folded block

Keeps every lattice reachable, but the format and the errors stay as they were, and a
hidden row still refuses a value shown in another.

### State the wall and derive the cell

Says straight away whether the lattice prints, but a scaffold is stated by its cell, and
the same wall makes very different cells at different densities.

### The option that won, and what it costs

A project's holes move on opening, so a project hollowed again comes out a little
different from the one it was saved from, and a user who tuned holes by hand loses them.

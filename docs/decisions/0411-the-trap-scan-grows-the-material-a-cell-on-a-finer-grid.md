# 0411. Grow the material a cell before reading the air, on a 0.05 mm grid

- **Status:** Proposed
- **Date:** 2026-11-07

## Context

`TrapScan` (ADR 0072) reads each layer onto the support grid, 0.1 mm a cell, lighting a cell
where the layer covers its middle, and joins the air runs that touch across a row or a layer.
A wall thinner than about two cells reads in steps: on a slant, one row's cell sits beside the
next row's only at a corner, and the air on both sides of the wall touches across it. The
outline is also simplified by half a cell before it is read, which thins a wall further.

A shell hollowed to 0.15–0.4 mm, which the Hollow tool allows and the printer's 20 µm pixels
print, then read as draining: the Crustling at 0.15, 0.2 and 0.4 mm had its whole cavity joined
to the outside through one such corner, and the check said nothing was trapped. That fails
towards a print full of resin, which is the failure ADR 0072 chose not to have.

## Decision

The scan reads on its own grid of 0.05 mm, and grows each layer's material by one cell, a
cross of radius one (`Field::grown(1)`), before it takes the air. Two cells that meet at a
corner then share a side, so a wall the grid sees at all closes.

## Consequences

Every wall the grid touches in each row holds the resin in, down to a few hundredths of a
millimetre on a slant. The air reads a cell short of each side wall, so a pocket reads small:
4689 against 4751 mm³ on the Crustling at a 2 mm wall, 1.3 %. A channel narrower than three
cells, 0.15 mm, reads closed, where it was one cell before; the pocket behind it is reported,
a hole nobody needed.

Twice the rows cost nothing seen: a slice of a 4.1-million-face open model at half
scale and a 1 mm wall, checked as it is written, takes 12.5–12.8 s before and after.

A wall thinner than half a cell standing square to the grid still lights no cell and drains;
reopen this if a shell that thin becomes something a user asks for.

## Alternatives considered

### Light a cell wherever the layer covers any of it

Conservative rasterisation closes the same corners without a pass of its own, but the
rasteriser lights by the middle only and the supports read the same `Field`; a second
shading mode for one reader is more than the growth costs.

### Read on the printer's pixels

Exact for what prints, but the rows grow with the panel: 25 times the cells of the 0.1 mm grid
on a 20 µm machine, for an error the growth already removes.

### The option that won, and what it costs

The pockets read about a cell small at each wall, and narrow channels read closed sooner.

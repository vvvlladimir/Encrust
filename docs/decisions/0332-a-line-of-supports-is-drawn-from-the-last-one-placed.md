# 0332. Draw a line of supports from the last one placed, along the chord taken to the surface

- **Status:** Accepted
- **Date:** 2026-10-23

## Context

A row of supports under an edge, a rim or a blade was placed one click at a time, and the
gaps between them came out as even as the hand. A painted patch fills an area, not a line,
and its rim is wherever the brush stopped. The surface between two supports is the model's
underside, which a straight chord between them leaves wherever the part curves.

## Decision

- **Shift-click draws the line.** With Place in hand, a shift-click stands supports from
  the last one placed on the same model to the click, the last at the click. The resting
  cursor shows a dot where each would stand.
- **Even, no wider than asked.** The chord is cut into the fewest equal steps no longer
  than the line spacing, and `core_supports::inline` takes each step's point to the
  nearest surface (`on_model`). A point on a blocked face, or within half a spacing of the
  one before, is left out; so is any the window's `stands` refuses, as for one click.
- **The spacing is a tool value.** `SupportState::inline_spacing_mm`, 4 mm unless set, is
  saved with the project and read as the default from a file without it. Where a line
  starts is not saved: it is the last support placed in this session, kept in the model's
  own space so moving the model keeps it.

## Consequences

A row of evenly spaced supports is two clicks. A line over a part that folds away from the
chord bunches where the nearest surface jumps, which the crowding rule thins, and can skip
a fold the chord passes far from; reopen with a path over the faces if that is reported.

## Alternatives considered

### A path over the faces between the two contacts

Follows any fold, but needs the face adjacency and a shortest-path search per frame of the
preview, for a line the hand usually draws along one flat underside.

### Two picked supports and a button

Takes a pick, a move to the panel and a press for what a modifier does in one click.

### The option that won, and what it costs

The line starts from whatever was placed last, so starting one elsewhere means placing a
support there first; and the nearest-surface step can land on the far side of a thin wall
the chord runs inside.

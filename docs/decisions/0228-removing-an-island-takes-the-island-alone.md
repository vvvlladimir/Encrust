# 0228. Removing islands takes the island alone

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

ADR 0164 lets the fold remove islands instead of naming them, and judges the next layer
against the layer as written. What stood only on an island then cures over nothing in
turn, so a floating part went whole: a model's arm that starts over a gap was erased to its
shoulder by one click. Users ask for the island the check found to go, and nothing else —
the speck of resin that would stick to the film, not every layer that would have been
built on it.

## Decision

A removing fold takes out of each layer exactly the pieces that touch nothing in the layer
below **as cured**, and keeps the layer below as cured, islands and all, when it judges the
next one. What stood on an island is therefore floating, as it was before anything was
removed: it is not taken out. It is named once instead, on the layer over the island, as
**floating** — a risk of its own that fails the print as an island does, so the check still
points at it and offers to support it. `removed_islands` counts the pieces taken out, one
per island.

Taking out what is left hanging is a further step the user takes by hand, one layer at a
time: each **pass** takes the layer of every part left hanging that the last one named, as
though it were an island in turn, and names what is left over it. The project keeps the
count as `hanging_passes` beside `remove_islands`, absent and so none in a project written
before it; the command line has no flag for it yet.

## Consequences

The file keeps every layer the check did not name, so a part floating over sixty layers
loses its first layer and keeps fifty-nine over a gap, which the verdict names until it is
supported. Removing islands therefore cleans specks out of a file but no longer passes the
check on its own when a part stood on one. The count of ADR 0206 is still one per part,
since a part now loses one layer only. The Check tool offers a further pass only while
something is left hanging, and a pass takes one layer, so a part is never erased whole by
one click.

## Alternatives considered

### Take the floating part whole in one step

What ADR 0164 did. It clears the vat of everything that cannot print, but it erases far
more than was found, and a user reading "1 island" does not expect an arm to go with it.
A pass that took the hanging parts whole was tried as the second step and erased the arm
the same way.

### Take the island alone, and what it costs

What is left over the gap still prints over nothing and fails unless it is supported; a
user who wanted the part gone has to delete it from the model. The check names it as
floating, one more kind for every reader of risks to match on.

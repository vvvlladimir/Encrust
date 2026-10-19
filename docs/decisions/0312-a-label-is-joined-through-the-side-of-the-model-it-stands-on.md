# 0312. Join a label through the side of the model it stands on

- **Status:** Accepted
- **Date:** 2026-10-19

## Context

A label is joined with `combine_exact` (ADR 0304, 0305), and every pass through the engine
costs the faces of the whole model however small the letters: reading a million-face model
in takes about 1.2 s, a boolean with it about as long again, so writing on a dense scan took
two and a half seconds after the union was made to take the model in once rather than three
times. The letters touch only the faces near them.

## Decision

`combine_exact_toward(models, op, toward)` cuts the first model with `core_geometry::cut`
by a plane square to `toward` 0.5 mm under the lowest point of the others, sends only the
side they stand on through the engine with them, and stitches the answer to the other side
along the cut: the lower cap is the last `Cut::cap_faces` faces of the side below, a field
added for it, and the upper one the faces of the answer lying in the cut; the two must meet
point for point and walk every edge of the line back to each other. A torn model, one of
several shells, one whose side cut off holds more than half its vertices, a cut that leaves
a loop open and a stitch that does not close all have the whole model combined instead.
Union and difference only. The text tool passes the normal it was clicked on.

The cut was made sound first: a fill turned over whole rather than triangle by triangle, a
hole given to the smallest ring round it, a face lying in the plane given to the half it
looks out of, and corners ear clipping passed over put back. Before, about a third of the
cuts through the models at hand left edges walked twice one way or open.

## Consequences

On a million-face model a label is joined in 0.13 to 3.2 s where it took 1.6 to 4.3 s, as fast as
the model is sparse near the letters, and answers the volume the whole cut does to within a
millionth of a percent. Of 34 labels on six models, 21 went the short way; the rest were on a
model of several shells, or curved so far round that the plane took most of the model, and
cost a cut more than the whole join, a few per cent. The model gains a line of split faces
along the cut, coplanar and invisible. The stitch checks itself, so a wrong join falls back
rather than lands. Reopen if a model of several shells is the common case: its shells would
have to be unioned once, kept, and cut after.

## Alternatives considered

### A box round the letters

Would take less of a model the letters stand on edge-on, but its six cuts meet at its edges
and leave a face split on one side of a seam and whole on the other.

### Keeping the model read in between stamps

Saves the 1.2 s of reading in, not the boolean, and holds the engine's copy of the model,
some hundreds of megabytes, for as long as the tool is open.

### The option that won, and what it costs

A plane takes everything on the letters' side of it: a label in a hollow, or on a wall with
an arm in front of it, takes that too, and past half the model gains nothing.

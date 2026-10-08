# 0205. A surface closes when its edges pair off, and repair drops what no winding covers

- **Status:** Accepted
- **Date:** 2026-10-08

## Context

`MeshDiagnostics::is_closed` read "no boundary edge and no edge with three or more faces",
and the window calls a model broken, refuses it the section cap (ADR 0195) and offers the
repair of ADR 0194 and 0195 on that verdict. Two files met in one afternoon showed both
halves of it failing, and in each the whole model was condemned by a few edges out of
millions:

- a helmet of 406 113 triangles with 21 open edges and three edges where two sheets of it
  touch along a seam. Filling the holes closed the surface; the three seam edges are used
  four times each, twice each way, and each sheet still has an inside. Nothing in repair
  addressed them, so the model stayed broken after every repair and never got a cap.
- a calibration part of 1 131 660 triangles, every edge used twice, with a cluster of
  slivers 0.05 mm across left by a boolean. Twelve of its edges are walked twice the same
  way, so no winding covers them: the mesh is reported as not orientable, which neither
  `orient_outward` nor `fill_holes` can do anything about.

A count of faces at an edge does not say whether the surface closes. Walking an edge as
often one way as the other does: that is what makes the enclosed volume, the generalised
winding number of ADR 0054 and the cap's crossing count all well defined.

## Decision

**`MeshDiagnostics` counts `unbalanced_edges` — edges more faces walk one way than the
other — and `is_closed` is that count being zero.** A branching edge whose uses pair off
no longer opens a model; a boundary edge is unbalanced by construction, so the single test
covers both. A face with no area has no side to be on and counts as whichever direction
its edge is short of, because a plane cut leaves collinear slivers along what it splits.

**`core_geometry::remove_unbalanced_faces(&mut Mesh) -> usize` drops every face at an edge
that does not pair off**, repeating until a pass finds nothing, since dropping a face can
unbalance a neighbouring edge. What is left is wound consistently everywhere, and the holes
it opens are loops `fill_holes` closes. Faces along a boundary edge are kept: a hole is not
a tangle.

**The asked-for repair runs drop, orient, drop the unbalanced, fill, orient.** Winding is
made as consistent as the mesh allows before the tangles are judged, or a shell written
inside out would read as one tangle per edge instead of one shell to turn round.

**The project format goes to version 4**, since its stored diagnostics carry the new count
and an older file cannot supply it.

## Consequences

Both files come back sound from one press of Repair: the helmet loses no face and gains
seven patches, the calibration part loses 18 slivers of about 0.002 mm² and gains 22
patches, and its enclosed volume moves by 0.002 mm³. A model a seam runs through is no
longer called broken at all.

Repair now takes surface away that the user did not ask to lose. It is bounded — only
faces at an edge no winding covers — but it is not reversible, which is why it stays behind
the question of ADR 0194 and is reported face by face in the status bar.

Projects written by an earlier build are refused and have to be rebuilt from their models.

The signal to reopen this: a file where dropping the unbalanced faces eats a feature rather
than a sliver. That would mean the tangle has to be resolved by splitting the surface along
it rather than by removing it.

## Alternatives considered

### Splitting a branching edge into two manifold ones

The textbook repair: duplicate the vertices so each sheet keeps its own edge, leaving the
mesh strictly two-manifold and losing nothing. It needs the fans round each endpoint split
with it to avoid opening a crack along every other edge at that vertex, which is a good
deal of surgery for a property that neither slicing nor the cap ever needed.

### Reporting an unorientable patch and letting the model through

Twelve bad edges in 1.7 million really are negligible, and the window could have said so
and capped the model anyway. It would have meant a verdict with a threshold in it — how
many bad edges are too many — and a cap counted from crossings that are locally wrong.
Repairing the file instead keeps the verdict absolute.

### The option that won, and what it costs

`is_closed` now answers a question about winding rather than about how many faces meet,
so "closed" and "two-manifold" have come apart: a mesh can be closed and still branch, and
the window reports both. The reader of a diagnostic has one more distinction to hold, and
every stored project from before this is unreadable for the sake of one counter.

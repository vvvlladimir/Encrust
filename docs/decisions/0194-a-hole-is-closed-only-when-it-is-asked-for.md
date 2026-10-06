# 0194. A hole is closed only when it is asked for, by the triangulator the cut already uses

- **Status:** Accepted
- **Date:** 2026-10-06

## Context

ADR 0005 settled that import repairs only what follows from the geometry — welding and
orientation — and reports holes, branching edges and duplicate faces without touching them.
It also said the window would be able to "offer a fix next to each one". It never did: a
model with a torn surface reached the plate silently, behind a sign at the end of its row,
and the one visible difference between a sound model and a broken one was a tooltip nobody
opened. A model that is open where it should be closed slices into contours that do not
close, so what comes out is not the part.

Two things had to be decided together. Whether this project closes holes at all, since
ADR 0005 rejected hole filling as a heuristic worth keeping out of step 1. And, if it does,
who asks for it.

`core-geometry` already fills a flat ring with triangles: a plane cut caps its halves by
ear clipping through `earcutr` (ADR 0089).

## Decision

**`core_geometry::fill_holes(&mut Mesh) -> Filled` closes boundary loops, and nothing calls
it on its own.** It walks the edge table for edges used by one face, follows them into
loops against the winding of the faces that own them — so a patch is wound like the surface
it closes — and fills each loop with `triangulate`, the ring filler the cut's cap uses.
`Filled` counts the loops closed, the faces added and the loops left open.

**The one triangulator is shared.** `triangulate` moved out of `clip.rs` into
`triangulate.rs`, crate-private, and both callers go through it. There is no second ear
clipper in this workspace.

**A ring with no plane is left open, a ring a triangulation cannot cover is fanned.** The
plane comes from Newell's normal over the loop; a loop whose normal is zero is a line and is
counted in `loops_left`. A projection that folds over itself comes back from `triangulate`
short of `n - 2` triangles, and the loop is closed by a fan from one new vertex at its
middle instead: closed badly beats open.

**The window asks before it fills.** A model whose `ImportSummary` is not sound raises a
modal naming what is wrong in plain words, with `Repair` and `Keep as it is`. Repair runs on
a worker thread like an import (ADR 0038) and replaces the mesh. Left as it is, the model's
row reads `broken` in words rather than carrying a sign, the choice stays on that row's own
menu, and the Slice footer reminds the user how many broken models the plate holds.

**Welding a vertex is not a repair the window mentions.** An STL stores every triangle on
its own, so every STL merges thousands of vertices; a mark for that stood on every row and
taught the user to ignore the row's marks.

## Consequences

- Import still hands over exactly what the file described, plus welding and orientation.
  Nothing invents surface behind the user's back.
- The patch is flat. A hole across a curve is covered by a chord, which is visible on the
  model and in the slice; that is a repair, not a reconstruction, and the user can see it
  and undo it.
- Repair replaces the mesh, so supports, a cavity or a texture placed on a model before it
  is repaired go with the mesh they were made for. Answering the question when the model
  lands — which is when it is asked — costs nothing.
- `core-geometry` grows another operation over `Mesh`, which ADR 0005 already accepted as
  the cost of putting repair there. The signal to revisit is a fill that needs to know
  about layers or exposure, or a second kind of patch.
- Non-manifold edges, self-intersections and duplicate faces are still only reported. A
  model carrying them is named broken and stays broken, which is honest but not helpful.

## Alternatives considered

### Keep reporting only, as ADR 0005 left it

No new algorithm, no new failure mode. Rejected because the report has no action behind it:
a user whose scan is open can do nothing about it here, and the plate happily slices a
surface that is not closed.

### Fill every hole at import, without asking

Fewest clicks, and most files would come out better. Rejected because a patch is surface the
file never contained: a lampshade modelled open and a scan with a hole are the same mesh to
us and must not get the same silent treatment.

### Minimum-weight triangulation, or a Liepa-style patch that refines and smooths

Better patches on curved holes, which is what a scan leaves. Rejected for now: it is a
tuned heuristic with its own failure modes, where ear clipping is the one already in the
crate and already exercised by every cut. The signal to reopen is a hole big enough that a
flat chord changes the part.

### This decision, and what it costs

A fan from a new middle vertex can self-intersect on a tangled loop, and nothing here
checks that it does not. The alternative is leaving the loop open, which fails the one
thing the user pressed the button for, so the fan stands — but a repaired mesh is not
promised to be sound, only closed where it could be. `fill_holes` therefore reports what
it did and the diagnostics are run again afterwards rather than assumed.

# 0195. Repair drops the faces drawn twice, and an open model is never capped

- **Status:** Accepted
- **Date:** 2026-10-06

## Context

ADR 0194 put a repair behind a question: a model that will not slice as it stands raises
it, and `fill_holes` closes the boundary loops when the user asks for it. Two things that
decision did not cover turned up on the first file it met, a cube written with one triangle
of a side missing and one of its top written twice.

Closing the holes did not make that model sound, so the window went on calling it broken
after it had been repaired: the duplicate face was still there, and the three edges it
doubled still read as branching. A repair the user cannot see the end of is worse than
none.

The same file also drew wrong. The section cap (ADR 0074) fills the cut wherever the
crossings a view ray makes above the plane say it is inside material. A hole swallows one
of those crossings, so behind the model — where the ray enters the front wall and never
leaves through the missing one — the count read "inside" and the cap was painted out into
the air beside the model, as a wedge standing on nothing.

## Decision

**`core_geometry::remove_duplicate_faces(&mut Mesh) -> usize` keeps the first face over any
three vertices and drops the rest, winding ignored.** A face laid over another is never
material twice; it only makes the edges round it read as branching. The duplicate indices
come from `validate::duplicate_faces`, which is what `diagnose` counts with, so what is
removed and what is reported can never disagree.

**The asked-for repair runs it before the filling**: an edge a duplicate has tripled is
neither a boundary nor a manifold edge, so the boundary walk cannot get through it. The
order is drop, fill, orient.

**A model whose `ImportSummary` is not sound counts nothing into the section cap.** It is
still drawn and still cut at the plane; only the stencil pass leaves it out. Crossings
describe an inside on a closed surface alone, so a model without one is shown open rather
than capped from a count that means nothing.

## Consequences

- A model whose only defects are holes and duplicates comes back sound, and the window
  drops every mark on it: no `broken` in the list, no warning tint in the viewport, nothing
  in the Slice footer. Red means a model nobody has repaired, or one repair could not mend.
- Scrubbing the section slider through a model left broken shows its inside walls instead
  of a cap. That is what it is: an open surface has no inside for the plane to be in.
- What repair did is said once, in the status bar. The import summary keeps describing the
  file as it arrived, so a mended model carries no mark of its own.
- Self-intersections and edges where three faces genuinely meet are still only reported,
  and a model carrying them stays broken and uncapped.

## Alternatives considered

### Leave duplicates alone and let the model stay broken

Honest about the file, and the fewest moving parts. Rejected because it makes the Repair
button lie: the user presses it on a model the window called broken and the model is still
called broken afterwards, with nothing said about why.

### Trim the cap quad to the models' footprint instead

Would stop the cap spilling into open air without knowing anything about soundness.
Rejected because it only moves the lie inside the silhouette: the count is still wrong
over the model itself, and the cap would be filled or missing in patches there.

### This decision, and what it costs

Dropping a face is destructive in a way welding is not: a file that deliberately carries
two coincident faces — a zero-thickness fin written twice — loses one of them, and we have
no way to tell that file from a broken export. The bet is that a printed part has no such
fin, and the escape is that the user asked for the repair and can undo it.

Leaving a broken model out of the cap also costs the one picture that would have been
useful: a user scrubbing a nearly closed scan gets no cross-section at all, where before
they got one that was right in most places and nonsense in a few. Showing nothing is the
honest half of that trade, and repairing the model gives the cap back.

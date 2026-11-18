# 0448. Sign by pseudonormal only on one manifold shell, and keep only the cavity inside the model

- **Status:** Accepted
- **Date:** 2026-11-16

## Context

A hollowed `niko.stl` came out with a bevelled cube one lattice tile wide beside the model,
and at precision 1 with a second cavity wrapped round the whole model: 3.56 million faces,
the resin saving overstated by 14 %. The mesh has no unbalanced edge, so `SignMode::Auto`
signed it by the pseudonormal (ADR 0055), but it also carries a two-sided film with no
thickness and 185 branching edges. The pseudonormal of Bærentzen and Aanæs is defined only
on a closed, oriented 2-manifold that does not cross itself; along the film's rim its two
sides cancel and the sign is noise. `crustling.encrust`, two shells crossing, failed the
same way. ADR 0055 named this as the signal to reopen it.

One wrong answer then spread. `reach` hands its side to every neighbour further than a
tile's diagonal from the mesh without asking again, so a wrong "inside" walked round the
model at the wall's distance. A column's gap between two stored tiles was decided by one
more independent question. And nothing looked at the cavity once it was meshed: a piece
outside the model, a loose solid island in the cavity and pockets of a hundredth of a
cubic millimetre all went into the shell and into `cavity_mm3`.

## Decision

- `Auto` takes the pseudonormal only when `diagnose` finds one shell, closed, with no edge
  shared by three faces or more; anything else is signed by the winding number.
- In `reach` a tile that would be stored, read either way, asks its own side; only the
  tiles walked through inherit one. A gap between stored tiles takes the side the walk
  gave its lowest tile, and asks only where the walk never reached.
- `outer_shells` leaves out of the field a shell enclosing less than a micron over its
  area: it bounds no solid. It stays in the model and prints as before.
- After `extract`, a shell's cavity is split into pieces. A piece wound against the rest
  is dropped unless it touches a blocker. A piece smaller than a ball of the default drain
  hole, 3 mm across, 14.1 mm³, is dropped and that resin stays solid. When two pieces or
  more remain, a piece most of whose faces the model's winding number calls outside is
  dropped, unless none is left, when the largest stays.

## Consequences

On `niko.stl` the stump, the phantom shell and the six pockets are gone at every precision;
`crustling` keeps one cavity and `bob` sixteen instead of twenty-one. A multi-shell model
now pays the winding number: `niko.stl` at precision 0.5 builds its field in 4.5 s instead
of 3.4 s. At precision 1 the run is faster, because the phantom shell no longer has to be
carved: `carve` 1.0 s instead of 3.3 s, the field 122 MB instead of 240 MB.

The filter costs a split of the cavity per carve; the winding hierarchy is built only when
a second piece above the pocket size is left to judge. The pocket size does not follow
the hole a user drills: the cavity is cut before any hole, and a cavity that changed with a
tool's setting would have to be cut again each time it moved.

Reopen this if a model with two clean shells side by side turns out common enough for its
winding-number cost to matter, or if a pocket under a drain ball is ever the one a user
meant to hollow.

## Alternatives considered

### Keep the pseudonormal and only filter the pieces

The filter alone removes every piece seen here. But a wrong side inside a stored tile also
moves the wall of the cavity that is kept, which no filter of pieces sees.

### Detect crossing shells and keep the pseudonormal on clean multi-shell meshes

Exact, but it is a self-intersection test over millions of faces on every build, to save
the winding number on a case the window rarely meets.

### The pocket size taken from the drill tool

What the user drills is known only to the window's tool and changes while the cavity
stands; tying the cavity to it would rebuild the shell on every change of the tool.

### The option that won, and what it costs

Every multi-shell or branching mesh is signed at the winding number's price, a third more
field time on the file that prompted this. A loose island a blocker did not make is
dropped even if a user wanted it: with blockers placed from the surface there was no such
case. A pocket under 14.1 mm³ is solid whatever drain the user plans.

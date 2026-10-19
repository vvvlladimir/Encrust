# Joints

How `core_joint::join` finds room on a cut face, sizes what goes there and builds it into
the two halves. Why it is done this way is in ADR 0306.

## The face on a grid

The lower half's cut face is read back from its triangles: those lying on the plane and
facing along its normal. Their edges used only once are the face's outline. The outline is
filled into a square grid in the plane by the even-odd rule, one row at a time. Each cell
is as fine as 0.05 mm, no coarser than 0.25 mm for pegs or a twelfth of a tongue's width,
and never more than 1024 cells across the face.

Two exact Euclidean distance transforms, one to the outside and one to the inside, give
every cell centre its signed distance to the edge. Each is one pass down the columns and one
along the rows, over the lower envelope of the parabolas rooted at each cell
(Felzenszwalb–Huttenlocher). Half a cell is taken off, since the edge lies between a cell
and its neighbour across it. Four-connected flood fill splits the face into its separate
pieces, so a cut through two legs gives two.

In a hollow model, pegs are placed on the face of the model's outside rather than the
shell's: a shell wall is too thin for a peg, and a socket over the cavity stands on a
pillar (below).

## Where fixings go

A fixing needs `reach` from its centre to the edge, and `gap` to the next fixing:

- `reach` = socket radius + 0.8 mm of wall, where socket radius = peg radius + clearance;
- `gap` = 2 × `reach` + 0.5 mm, so two sockets, or two pillars, never meet.

The candidates on each piece are the cells at least `reach` from the edge. A single fixing
takes the deepest one. Several take, first, the cell furthest from the candidates' middle
(one end of the piece), then each time the cell furthest from those already taken (the
other end, then the widest gaps). A candidate is tried before it is taken. Turned down, it
and everything within `reach / 2` of it are dropped, and after 64 refusals the piece is
given up.

The try sounds the model from the spot into each half: nine rays through the `Bvh`, one at
the centre and eight round the socket's rim, the least distance counting. The ray runs
against the uncut model, so the first hit is the far surface. A peg is shortened to what
the depth allows after 0.8 mm of floor, the clearance and 0.2 mm of glue space, and the
spot is refused if that leaves less than half the peg:

| Kind | Socket depth in the lower half | Socket depth in the upper half |
|---|---|---|
| Pins | — (the pin sinks 0.1 mm into it) | length + clearance + 0.2 |
| Dowels | length / 2 + clearance + 0.2 | length / 2 + clearance + 0.2 |
| Magnets | thickness + clearance | thickness + clearance |

A magnet is never shortened: a spot not deep enough for it is refused.

An automatic peg is half as wide as the widest circle its piece holds, rounded down to
0.5 mm and kept within 2–6 mm, and half as long again as it is wide. A keyed set makes its
first peg 0.7 times as wide, at least 0.75 mm, so no turn of one half maps the sockets
onto the pins.

## Pillars

In a hollow model a socket may fall over the cavity, or on a wall too thin to hold it. That
happens where the shell's own face is less than `reach` from its edge at the spot, or the
shell is shallower there than the socket and its floor. A bar of radius `reach` then runs
from the cut into that half, to 0.5 mm past the furthest of the nine rays against the
outside. It is intersected with the model's outside, so it never shows through the
surface, and joined to the shell before the socket is taken out of it.

## The shapes

Pins, dowels, sockets and pillars are solids of revolution, their flats within 0.01 mm
of the circle and never fewer than 24. Pins and dowels have one side cut flat at 0.85 of
their radius, which vents a blind socket of air and spare glue. Their ends are chamfered
by a quarter of the radius, at most 0.4 mm. A socket's mouth is chamfered at 45°, by a
fifth of its radius and at most 0.3 mm, against the flare of the first layers. A socket's
cutter starts 0.5 mm outside its half, so no face of it lies on the cut.

## The key

The tongue is the band of the shell's own cut face from `inset` to `inset + width` in
from its edge. That is a rim inset round a solid face. On a wall narrower than the band
needs, `inset` shrinks so the tongue keeps to the wall's middle, but never below 0.4 mm of
wall outside the groove. The groove is the same band widened by the clearance on each side,
and deeper by the clearance and 0.2 mm.

A band is traced by marching squares over `min(d − from, to − d)` at the cell centres.
Each cell's inside is a polygon of its inside corners and the edge crossings between them.
A crossing never comes within 2% of a corner. Points are keyed by their place on the grid,
so neighbouring cells share them. A saddle is joined through the cell's middle when the
middle is inside, and split into its two corners when not. The traced triangles are laid
at the band's foot and its top, and walled round every directed edge that has no twin.

## Building

Each half is read into a `Solid` once. It is joined with its pillars (intersected with the
model's outside), then with its pins or tongue, and its sockets or groove are taken out.
Each set goes in as one mesh of many shells. Fixings stand `gap` apart, so no two shells
of a set cross. A half with nothing to do is handed back untouched.

# The shortest walk over a surface

`core_geometry::surface_path` finds the shortest way between two points of a mesh without
leaving its surface: Measure along the surface in the window, `encrust measure
--along-surface` on the command line. Why this method: ADR 0400.

## Corridor

A* runs over the faces. A face is entered across one of `CROSSINGS` (three) points evenly
spaced on an edge it shares with the face before; a step costs the straight line from where
the face was entered to the next crossing, and the estimate adds the straight line to the
end, which never overstates, so the first time the end's face comes off the queue its
corridor is the cheapest found. With the midpoint alone the corridor strays from a coarse
ball's meridian by 10 %; three points keep it within the proptest's 5 % of the great-circle
arc. A step never crosses back over the edge it came in by. Two ends on shells no edge
joins are refused before the search, by a label per face that `Adjacency::joins` works out
once per mesh. Otherwise the walk visits at most the faces of the shell both ends are on,
and in practice an ellipse round the straight line between them; near opposite ends of a
round body that is most of it, since the straight line runs through the inside.

Every 4096 nodes the search tells its watcher the share found — how much nearer than the
start it has come to the end in a straight line — and stops when the watcher answers
`Break`. An estimate weighted above the straight line was tried and dropped: at 1.1 it
already bends the corridor off a cube's exact walk.

## Unfolding

The corridor is laid flat face by face: the first triangle at the origin, each next one
hinged onto the edge it shares with the one before, its third corner placed by its two
distances to that edge's ends on the side away from the previous third corner. Lengths are
kept, so a straight line in the flat strip is a walk over the faces as long as it is. The
ends are placed by their barycentric coordinates in the first and last face, then moved
`INWARD` (1e-5) of the way to their face's centre: a start on an edge, which Edge snapping
always gives, would sit on its first portal and leave the funnel no width. Far from the
origin that move rounds away in `f32`; an end left on its last portal is still inside the
funnel, which ties keep open.

## Funnel

Each shared edge is a portal, its ends named left and right as the walk meets it. The simple
stupid funnel algorithm (Mononen, 2010) pulls the line taut through the portals: it keeps an
apex and a left and right side, narrows each side while the next portal stays inside, and
when a side would cross the other, that side's end becomes a corner of the walk and the scan
restarts behind it. A restart always moves the apex to a later portal, so there are fewer
restarts than portals.

Where the taut line crosses each portal is found in the flat strip as a share of the edge,
then taken back to the placed edge: the walk is a polyline with one point on every edge it
crosses, lying on the surface.

## Limits

The walk is the shortest within its corridor, exact on a strip that unfolds flat (a cube's
faces, a cylinder's side) and a little long where the true shortest walk would leave it.
A mesh placed with a scale is walked where it stands, so lengths are the plate's.

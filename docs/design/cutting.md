# Cutting and splitting

How `core-geometry::cut` takes a mesh apart on a plane, and `split` takes it apart by
what it is joined to. Why here and not through a field: `docs/decisions/0089`.

## The cut

Each vertex gets its signed distance to the plane, and each face a sign a corner: above,
below, or on it within 0.1 µm. A face whose signs are all one way goes to that half
whole; a face lying in the plane goes to the half it looks out of — the lower half for a
face looking along the normal — and is that half's surface there.

A straddling face is clipped into the polygon on each side — four corners at most, always
convex, so it is fanned rather than triangulated. The points the clip makes are keyed by
the edge they came from with the lower vertex index first, so the two faces sharing that
edge make one point: the section's loops then close on exact indices, with no tolerance
anywhere. A point the plane passed through a vertex at is that vertex in the half that
keeps it, rather than a second one on top of it.

Nothing straddles where the plane holds a whole edge, as it does at the ring of vertices
around a lathed model's equator. Such an edge is still where the section ends, so it is
collected from the faces on either side and handed to the section whenever the material
swaps sides across it. A face lying in the plane counts as a face of the half it went to,
so the section runs round a step whose top is in the plane as round any other edge; an edge
both of whose faces lie on one side bounds no section.

## The cap

The clipped polygons hand back their cut segments in the order the material below the
plane runs, which is what gives the loops a consistent winding. Segments are walked into
rings; a chain that never returns to its start is counted in `Cut::open_loops` and
dropped, because a model with a hole in it has no section to close.

Rings are projected into the plane's own basis. The largest one says which way the outside
winds; anything winding the other way is a hole in the smallest outer ring round it — not
every one, since a hole in an island in a cavity lies inside the outside wall too, and would
be filled twice. `earcutr` fills each outer ring with its holes, and the fill comes back
wound as its rings are, so the cap is turned over whole by its area: the lower half's cap
faces up and the upper half's down. Turning each triangle by its own normal instead leaves a
sliver too thin for its sign facing against its neighbours, the edges between them walked
twice one way, which on a dense scan was one cut in two.

Two corrections stand between ear clipping and a cap that closes. A corner standing on the
one before it — which a section has wherever the model touches itself on the plane — is
taken out of the ring before the fill, because ear clipping drops it instead of covering
the two edges that reach it, and it comes back as one triangle over those edges, wound
against the edge the fill left behind. Three corners in a straight line give an ear with no
area, which covers the ring but cuts into two coincident points when a plane crosses it
later; each one is flipped into the triangle across its longest edge, the usual diagonal
flip, which is safe because a flat triangle and its neighbour always cover a convex
quadrilateral. And ear clipping passes over corners lying exactly on a line between two
others — along a straight side, or where it bridges a hole along a side the ring runs on
— leaving an edge of the fill with nothing beyond it that is no side of a ring: the corners
on it whose own sides went uncovered are put back by fanning the triangle over it out to
each.

## The split

Union-find over the faces' vertex indices: two faces are in the same piece when they share
one. Pieces come back biggest first. A mesh has to be welded before this means "separate
parts" — two copies sharing a face's position but not its indices are two pieces until
`weld` makes them one.

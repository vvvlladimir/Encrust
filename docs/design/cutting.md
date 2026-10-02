# Cutting and splitting

How `core-geometry::cut` takes a mesh apart on a plane, and `split` takes it apart by
what it is joined to. Why here and not through a field: `docs/decisions/0089`.

## The cut

Each vertex gets its signed distance to the plane, and each face a sign a corner: above,
below, or on it within 0.1 µm. A face whose signs are all one way goes to that half
whole; a face lying in the plane goes to the lower half, which is the half it closes.

A straddling face is clipped into the polygon on each side — four corners at most, always
convex, so it is fanned rather than triangulated. The points the clip makes are keyed by
the edge they came from with the lower vertex index first, so the two faces sharing that
edge make one point: the section's loops then close on exact indices, with no tolerance
anywhere.

## The cap

The clipped polygons hand back their cut segments in the order the material below the
plane runs, which is what gives the loops a consistent winding. Segments are walked into
rings; a chain that never returns to its start is counted in `Cut::open_loops` and
dropped, because a model with a hole in it has no section to close.

Rings are projected into the plane's own basis. The largest one says which way the outside
winds; anything winding the other way and lying inside it is a hole. `earcutr` fills each
outer ring with its holes, and every cap triangle is wound by testing its normal against
the plane, so the lower half's cap faces up and the upper half's faces down.

## The split

Union-find over the faces' vertex indices: two faces are in the same piece when they share
one. Pieces come back biggest first. A mesh has to be welded before this means "separate
parts" — two copies sharing a face's position but not its indices are two pieces until
`weld` makes them one.

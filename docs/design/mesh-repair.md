# Mesh repair and inspection

What `core-geometry` does to a freshly imported mesh, in this order: weld, inspect,
orient — and, when the user asks for it, close what is still open. Why it lives there:
ADR 0005, ADR 0194 and ADR 0195.

## Why welding comes first

STL has no vertex table: a shared corner is written once per face, so a 12-triangle cube
arrives as 36 vertices, every edge is used by one face, and the topology is worthless.
Exporters also round per face, so one corner comes out `9.999999` and `10.000001` and
exact matching leaves hairline cracks that look like holes. `stl_io` deduplicates by bit
pattern, which is why `StlLoader` bypasses it and returns raw soup: one welding step with
one tolerance is easier to reason about than two, and the vertex count then tells the
truth about the file.

## Welding

`weld(mesh, tolerance)` merges vertices closer than `tolerance` millimetres.

A uniform grid hashes each vertex to a cell and stores only cell *representatives*, so a
match lies within one cell on every axis and no chaining is possible — without that, A
merging into B and B into C could leave A and C further apart than the tolerance. Every
merged vertex is within `tolerance` of its representative, which `tests/invariants.rs`
pins down.

The cell is **2.5 tolerances** across. At that width the half of the cell a vertex sits in
on each axis decides which side a match can be on, so eight cells are worth probing
instead of 27 — a match in any other would be at least 1.25 tolerances away. Cells are
hashed with `FastHasher` rather than `SipHash`: a cell derived from our own vertex is not
an attacker's key, and there are millions of them. A cell's representatives are searched
in the order they were kept.

Few vertices ever reach the grid (ADR 0271). The exact copies go first, through a parallel
sort over bit patterns: an exporter writes a shared corner with the same bits every time,
so this leaves about a sixth of an STL's vertices. Of those, a point with no other point in
its eight probe cells can merge with nothing, and no point can merge into it. Such points
are found in parallel by binary search in a sorted list of occupied cells. Only the rest,
with every copy of them, go through the grid in their original order, which is why the
result is exactly what one vertex at a time gave.

The cell has a floor of `1e-7` mm so a zero tolerance still hashes finitely, which also
puts `-0.0` and `0.0` in one cell where exact comparison would not.

After remapping, a face whose three indices are no longer distinct is dropped, as is one
referencing a vertex that does not exist — which is how a truncated file degrades instead
of panicking.

The default tolerance is `1e-5` mm: three orders below what a resin printer resolves, well
above the f32 noise accumulated across a 200 mm plate.

## The edge table

Everything topological comes from one sorted array. Per face and corner, an `EdgeUse`
records the two vertex indices in ascending order, the face, and whether that face walked
the edge low-to-high. Sorting groups an edge's uses, so one pass yields every fact:

- one use: a boundary edge, the surface is open;
- two: an ordinary manifold edge;
- three or more: the surface branches.

Branching is not the same as being open. Two solids meeting along a seam use its edge four
times, twice each way, and each of them still has an inside. What decides whether a surface
closes is whether every edge is walked **as often one way as the other** — that is what
makes the enclosed volume, the winding number of ADR 0054 and the section cap's crossing
count well defined, and `is_closed` is that and nothing else (ADR 0205). A face with no
area has no side to be on and counts as whichever direction its edge is short of, because
a plane cut leaves collinear slivers along every edge it splits.

The sort is a counting sort on the edge's lower vertex, then an ordinary sort within each
run. Every use of one edge shares that vertex, so runs are short — the valence, about six
— and the result is the full sort's order at linear cost in faces. The array costs about
48 bytes per triangle and is built once, deliberately allocation-flat: a
`HashMap<Edge, Vec<Face>>` would allocate a `Vec` per edge, which at a million triangles
dominates everything.

Two faces sharing an edge are consistently wound when they walk it in **opposite**
directions. That single predicate drives the orientation pass.

## Diagnostics

`diagnose` reports counts, not verdicts, because the CLI and the window react differently:
the edge counts — boundary, branching and unbalanced — degenerate and duplicate faces,
unreferenced vertices, connected shells, and the Euler characteristic `V - E + F` over
referenced vertices.

Euler is the cheapest sanity check there is — 2 for a closed shell with no handles, 1 for
a disc — and anything else on a closed mesh means the counts disagree with each other.

A face is degenerate when two indices are equal or the cross product is exactly zero.
Slivers of tiny but non-zero area are not counted: that threshold is a slicing concern and
lives with the plane-intersection epsilon.

## Orientation

`orient_outward` makes winding consistent, then turns closed shells the right way out. STL
face normals are never consulted — exporters write them wrong often enough that winding
order is the only trustworthy signal.

1. Link every two faces sharing an edge no third face uses, tagging the link with whether
   they already agree.
2. Union the agreeing links into patches. A patch is turned as a whole, so a conflict
   inside one — a half twist, or two bodies' walls folded onto each other — is left where
   the file put it rather than pushed around by a walk.
3. The disagreeing links between two patches form a seam. Seams are honoured heaviest
   first through a union-find that keeps parity: joining two patches ties them to opposite
   turnings, and a seam whose patches are already tied is skipped.
4. Within a group of tied patches only the two assignments matter, so the one turning
   fewer faces wins: the repair is minimal, and without it a cube with three inverted
   faces would be "fixed" by flipping the other nine.
5. If the turning leaves as many seam edges in conflict as the file had, or more, the
   file's winding stands. Orientation never makes a file worse (ADR 0269).
6. Per closed component, sum `a · (b × c)`: six times the enclosed volume, negative
   exactly when the shell is wound inwards. Open components are skipped — with a hole
   there is no inside to be on the wrong side of.
7. A closed shell inside no other is flipped when its volume is negative. One inside
   another follows its holder, the smallest closed shell around it: flipped when the
   holder was, kept otherwise. So a cavity wound inward stays a cavity, a body inside a
   body stays a body, and a hollow model inside out turns with its cavity (ADR 0358).

The holder is found only where some closed shell is wound inwards, as with every one wound
outwards nothing turns. Shells boxed inside another's bounds, found by a sweep along x, cast
a ray along each axis from a point on them through a hierarchy of the boxing shells' faces;
an odd count of crossings with a shell on two of the three rays puts them inside it.

Steps 6 and 7 are why `Orientation` reports `inverted_shells` apart from `flipped_faces`: a model
entirely inside out is a different problem from a few stray faces. `orientable` is false
when a conflict is left, which is a note and not a defect: the fill rule takes what each
body encloses (ADR 0269).

## Faces laid over others

`remove_duplicate_faces(mesh)` looks at faces over the same three vertices. A pair wound
opposite ways is a wall of no thickness that encloses nothing, so it goes whole. Dropping
one of them would leave the other walking its edges the same way as a neighbour. Of a stack
wound one way, the first face stays. Neither changes what the surface encloses, so import
runs it after welding and before orientation, which would otherwise read a doubled edge
as a branch (ADR 0270). The faces are found by the same counting sort as the edge table,
keyed by sorted corners.

## Closing holes

Repair adds surface the file never had, so it runs only when the user presses Repair
(ADR 0194, 0270). The order is collapse the faces with no area whose corners stand on one
point (ADR 0310), drop the doubled faces, stitch the seams (ADR 0350), drop the specks,
orient, fill, drop the doubled faces again, orient. A speck goes before the fill, which
would otherwise close it into a shell that looks kept. If the result has more boundary edges, degenerate faces and
duplicates between them than the input had, the input stands.

An edge used an odd number of times cannot pair off without one more face. A hole's edge
is used once, and the foot of a fin three times. The patch walks each such edge the way
the surface is short of, so the loops it follows are wound like the surface around them,
and `orient_outward` afterwards has nothing to flip. A fin's loop is closed by the fin's
own reverse, which the second drop of doubled faces cancels against it.

A loop of up to 256 corners is patched by the triangulation of least total area, found by
dynamic programming over its spans: the cost of a span is the cheapest split of it into
two smaller spans and a triangle. A triangle whose doubled area is under `1e-6` of its
longest edge squared is a sliver and is not allowed. On a flat loop, only a triangulation
that does not overlap itself reaches the loop's own area. On a curved loop, the result is
the tightest skin across it, not a fair continuation of the surface.

A longer loop, or one every triangulation of which holds a sliver, is flattened into the
plane of its Newell normal and ear clipped by `triangulate`, the `earcutr` call a plane cut
caps its halves with (ADR 0089). If that comes back short of `n - 2` triangles, or with a
face of no area — corners in a row that the projection rounded into an ear — the loop is
fanned from one new vertex at its middle. A Newell normal of zero means the loop is a line,
which is left open and counted in `loops_left`.

Other methods run part of it (ADR 0351): *close holes* drops the doubled faces and
patches, without collapsing, stitching or dropping specks; *close small holes* patches only
the loops of three corners a single missing face leaves, and turns nothing; *turn* only
orients. Each keeps the never-worse rule.

## Dropping specks

`drop_small_shells(mesh, smallest_mm2)` drops a shell, faces joined across shared edges, when
it has an edge an odd number of faces use and its faces add up to under `smallest_mm2`
(1 mm² by default). A closed shell stays however small: a thin pin is part of the model,
and only the user can say it is not.

## Stitching seams

Two patches of one surface meshed at different densities meet in a seam: the coarse side's
edge `a → b` has the fine side's vertices standing on it, and both are open. Patching it
lays slivers along the crack; `stitch_seams(mesh, tolerance)` closes it instead by fanning
the coarse face from its far corner through those vertices. No vertex is made or moved.

A vertex is laid into an open edge when it lies strictly between its ends, within the
tolerance of it, and ends an open edge walked the other way along the same line. The last
condition keeps a sheet merely standing on an edge from being sewn into it. A split that
would leave a face with no area is skipped. The open corners are hashed on a grid with
cells of the mean open edge, at least two tolerances, and each edge is walked in half-cell
steps over the 27 cells around each, so the cost follows the open edges' total length.
A face has one edge split a pass; four passes cover its three edges.

`DEFAULT_SEAM_TOLERANCE` is 0.01 mm. On the test models it closes every crack of one
statuette with no patch at all, and on a 4-million-face colour mix it stitches about 1 900
edges the fill would otherwise have bridged.

## Crossing faces

`self_intersections(mesh, bvh)` lists the pairs of faces that cross. Each face asks the
hierarchy for the faces whose bounds meet its own (`Bvh::faces_meeting`), on rayon, and
keeps those after it that share no corner and pass a strict test in `f64`: an edge of one
goes through the inside of the other, or, all in one plane, two edges cross or a corner
stands inside. Faces that only touch are not a pair, so neither is a crossing that lies
exactly along an edge. It is reported, never fixed: wrapping is what removes crossings.
200 k faces take 25 ms (`benches/mesh_repair.rs`).

## Wrapping

`core-volume::wrap` repairs the mesh, then unions its shells with the exact engine. The
result is taken if it is sound and crosses nothing; else only the repair stands and the
report says the wrap was left out. One sound shell crossing nothing is already its own wrap
and stands as it is. A shell crossing itself skips the engine, which returns it still
crossing. *Deep* is repair, wrap, repair, and reports what both repairs did; unwrapped, the
second repair would meet the first one's mesh, so it is not run. A shell inside another
wound outward is a body the wrap swallows; one wound inward is a cavity, the engine reads
the mesh whole and keeps it (see Orientation).

There is no sampled fallback (ADR 0357): on all 31 test models the field's surface pinched
thin parts into branches and was refused. Exact unions of separate closed shells are what
wrapping does well today.

## Placement

`transform_mesh` applies scale, then rotation, then translation. A mirroring scale — an
odd number of negative components — reverses winding, so faces are flipped to keep normals
out; otherwise mirroring would invert every shell and the orientation pass would dutifully
undo it.

`drop_to_plate` and `center_over_plate` return translations rather than applying them, so
a caller composes them in one pass over the vertices. Neither knows the plate size: the
caller passes it, which keeps `core-geometry` clear of `printer-profiles`.

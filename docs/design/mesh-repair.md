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
instead of 27 — a match in any other would be at least 1.25 tolerances away. Over the 2.6
million vertices of an unwelded million-triangle STL, that is most of the cost. Cells are
hashed with `FastHasher` rather than `SipHash`: a cell derived from our own vertex is not
an attacker's key, and there are millions of them. The bucket map and output list are
reserved for a sixth of the input, which is about what an STL leaves.

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

1. Build face adjacency from the manifold edge groups, tagging each link with whether the
   faces already agree.
2. Breadth-first per component, assigning a flip flag: a face keeps its winding when it
   agrees with the neighbour that reached it. An assigned neighbour that disagrees means
   no consistent orientation exists — a Möbius strip — and the mesh is reported as not
   orientable.
3. Within a component only the two assignments matter, so the smaller one wins: the repair
   is minimal and independent of the seed. Without it, a cube with three inverted faces
   would be "fixed" by flipping the other nine.
4. Per closed component, sum `a · (b × c)`: six times the enclosed volume, negative
   exactly when the shell is wound inwards, so the shell is flipped. Open components are
   skipped — with a hole there is no inside to be on the wrong side of.

Step 4 is why `Orientation` reports `inverted_shells` apart from `flipped_faces`: a model
entirely inside out is a different problem from a few stray faces.

## Closing holes and dropping faces

These are the parts of repair nobody runs without being asked: one adds surface the file
never had, the others throw faces away, so the window puts the question to the user first
(ADR 0194, 0195, 0205). The order is drop the duplicates, orient, drop the unbalanced,
fill, orient.

`remove_duplicate_faces(mesh)` keeps the first face over any three vertices and drops the
rest, winding ignored. It goes first: an edge a duplicate has tripled is neither a
boundary nor a manifold edge, so neither the walk below nor the orientation pass can get
through it.

`remove_unbalanced_faces(mesh)` drops every face at an edge that does not pair off, and
repeats until a pass finds nothing, since dropping a face can unbalance a neighbouring
edge. What is left is wound consistently everywhere — an orientation can only fail where
two faces walk one edge the same way — and the holes it opens are loops the fill closes.
Faces along a boundary edge are kept: a hole is not a tangle. Orientation runs before it so
that a shell written inside out is turned round rather than taken apart.

The boundary edges — the groups of one in the edge table — are followed into loops
*against* the direction the face that owns each edge walks it. A patch triangulated in that
order is therefore wound the same way as the surface around it, and `orient_outward`
afterwards has nothing to flip.

Each loop is flattened into the plane of its own Newell normal, which is the area-weighted
normal of a ring that need not be flat and does not depend on where the ring stands. A
normal of zero means the loop is a line: there is no plane to lay triangles in, so the loop
is left open and counted in `loops_left`.

A flat ring is filled by `triangulate`, the same `earcutr` call a plane cut caps its halves
with (ADR 0089). It returns `n - 2` triangles for a ring of `n` corners; anything short of
that means the projection folds over itself, and the loop is closed instead by a fan from
one new vertex at its middle. The patch is flat either way: a hole across a curve comes out
as a chord, which the user can see and undo.

## Placement

`transform_mesh` applies scale, then rotation, then translation. A mirroring scale — an
odd number of negative components — reverses winding, so faces are flipped to keep normals
out; otherwise mirroring would invert every shell and the orientation pass would dutifully
undo it.

`drop_to_plate` and `center_over_plate` return translations rather than applying them, so
a caller composes them in one pass over the vertices. Neither knows the plate size: the
caller passes it, which keeps `core-geometry` clear of `printer-profiles`.

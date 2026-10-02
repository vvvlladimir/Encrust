# 0005. Mesh repair lives in core-geometry, and repairs only what is unambiguous

- **Status:** Accepted
- **Date:** 2026-09-17

## Context

A mesh coming out of an STL file cannot be sliced as it stands. STL has no vertex table, so
a shared corner is stored once per face that touches it and no two faces appear adjacent.
Until vertices are welded there is no topology at all: a 12-triangle cube reports 36
boundary edges. Everything downstream — closing contours in step 2, detecting overhangs in
step 7 — assumes adjacency that does not exist yet.

Three questions had to be answered together. Which crate owns this work. What counts as
"the same vertex". And how much the slicer is allowed to change about a model the user
handed it.

`core-slicer` needs the answer before it can cut anything, `core-mesh-io` needs it right
after parsing, and `core-supports` needs face adjacency to find overhangs. Three peers, one
shared need.

## Decision

Welding, diagnosis and orientation live in `core-geometry`, as free functions over `Mesh`:
`weld`, `diagnose`, `orient_outward`. That is the crate all three peers already depend on,
so by rule 5 the shared need moves down rather than sideways. Rule 4 is not strained: the
data types stay free of business logic and the algorithms are functions beside them.

**Welding uses an absolute tolerance, default `1e-5` mm, overridable per run.** A resin
printer resolves around `2e-2` mm, so the default is three orders of magnitude below
anything that could matter, and comfortably above f32 noise over a 200 mm plate. A tolerance
relative to the bounding box was rejected: the same part would weld differently before and
after a scale, which is exactly the kind of surprise that costs a print.

**`MeshLoader` implementations return the file's triangles unwelded.** `stl_io` offers a
deduplicating reader that matches on bit pattern; `StlLoader` bypasses it and reads raw
triangles instead. One welding step with one tolerance is easier to reason about than two
with different rules, and the reported vertex count then describes the file rather than an
intermediate step.

**Step 1 diagnoses everything and repairs only orientation.** Winding is made consistent
and inside-out closed shells are turned outwards, because both are derivable from the
geometry with no guesswork: adjacency fixes the first, the sign of the enclosed volume
fixes the second. Holes, self-intersections and T-junctions are reported and left alone.

**The CLI warns and exits zero.** `--strict` turns defects into a non-zero exit for scripts
that want it.

## Consequences

- `core-slicer` can demand a welded, oriented mesh in step 2 without depending on the
  importer, and `core-supports` gets face adjacency from the same place in step 7.
- Diagnostics are counts, not verdicts. The CLI prints them; the GUI will be able to
  highlight the offending edges and offer a fix next to each one.
- Every geometric invariant now has one obvious home, which is where the property tests
  point.
- The cost is that `core-geometry` is no longer only data. It has an algorithms half, and
  the boundary between "primitive" and "operation on primitives" has to be watched. The
  signal that it has gone too far is a function there that needs to know about layers,
  exposure or pixels.
- Welding allocates a hash grid and a full copy of the mesh. On a million triangles that is
  real memory, and the benchmark in `benches/mesh_repair.rs` exists to notice when it
  becomes a problem.
- The tolerance is a guess that has not met a real printer yet. If parts come out of step 4
  with hairline seams, this is the first number to question.

## Alternatives considered

### A dedicated `core-mesh-repair` crate

Cleanest by responsibility: `core-geometry` stays pure data, repair gets its own boundary
and its own tests. Rejected because it is a tenth crate holding three files, and because
`core-supports` and `core-slicer` would both have to depend on it anyway — the same
dependency edge, with an extra manifest in the middle. Worth revisiting if repair grows
hole filling and remeshing in a later step.

### Repair inside `core-mesh-io`, at load time

Tempting, because it guarantees nobody ever sees an unwelded mesh. Rejected because
`core-slicer` would then have to depend on the importer to validate a mesh it built any
other way — from a GUI boolean operation, from a support generator, from a test — and that
inverts the dependency graph. It also hides a lossy step inside what should be a parser.

### Full repair in step 1

Hole filling, self-intersection removal and T-junction stitching would make step 1 deliver
a mesh that always slices. Rejected because each of those is a heuristic with its own
failure modes and its own tuning, and bundling them would double the step while making
every failure harder to attribute.

### This decision, and what it costs

Welding before anything else is lossy and irreversible: two vertices `1e-6` mm apart were
possibly distinct by intent, and after welding that information is gone. For printed parts
this is meaningless, but it means `core-geometry` cannot be used for exact CAD-style
operations later without a separate unwelded path.

Repairing orientation silently is also a small liberty taken with the user's data. The
report says what changed, but a user who deliberately modelled inward-facing shells gets
them turned around without being asked. The bet is that such a user does not exist, and
that inverted normals from 3D scans are common enough to be worth fixing by default. The
escape hatch is `--no-validate`, which skips both the diagnosis and the repair.

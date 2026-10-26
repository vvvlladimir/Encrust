# 0353. A summary counts holes, and inspect counts crossing faces

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

`MeshDiagnostics` counted open edges but not the holes they make: 41 open edges may be one
hole or twenty, and a patch is per hole. The diagnostics are part of a project's model
summary, so a new count is a change to the project format.

Faces that cross each other are invisible to the diagnostics: a model of overlapping parts
reads sound. Only wrapping (ADR 0352) removes them, and the user needs a count to decide to
wrap.

## Decision

**`MeshDiagnostics` gains `holes`, the loops of open edges, read with `#[serde(default)]`** so
a project written before reads 0. `diagnose` walks the loops with the same walk the fill uses,
over the edge table it has already built. The text report names the holes among the defects,
and the JSON `repair` object carries `holes`.

**`core-geometry::self_intersections` lists the pairs of faces that cross**, through a new
`Bvh::faces_meeting` box query, strictly and in `f64`; see `docs/design/mesh-repair.md`. It is
not part of `diagnose`: `encrust inspect --intersections` asks for it, and the Repair tool
will on a press.

## Consequences

- An old project opens with no holes counted until the model is read again.
- 200 k faces cost 25 ms to check for crossings, 4 M faces about half a second.
- A crossing that runs exactly along an edge, or two faces only touching, is not counted.
- The signal to revisit is a crossing the count misses that slices wrong.

## Alternatives considered

### Count crossings in every `diagnose`

Always there for the window. Rejected because `diagnose` runs on every import and every
landed tool result, and most models need no such check.

### Store the hole loops themselves in the project

What highlighting them needs. Rejected: the window computes them on demand from the mesh it
already has, and a project should not carry what it can recompute.

### The option that won, and what it costs

A project field more, and a strict test that misses the degenerate crossings an exact
predicate would catch.

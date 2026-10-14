# 0269. Weld, index and sort a mesh's edges on rayon, with the same result one thread gives

- **Status:** Accepted
- **Date:** 2026-10-14

## Context

The geometry stage of opening a 1.13-million-triangle STL took 0.8 s on one thread of
eight:

| Step | Time |
|---|---|
| weld | 328 ms |
| hierarchy | 208 ms |
| orientation | 106 ms |
| `diagnose` | 90 ms |
| reading the file | 38 ms |

ADR 0038 moved that work off the frame, but the user still waits for it. Rule 7 of
`AGENTS.md` asks that work proportional to triangles run on rayon. `core-geometry` was the
one core that did no such work in parallel and depended on nothing that could.

Most of welding's time was not spent merging anything. An exporter writes a shared corner
with the same bits every time it uses it. Of the vertices left once those copies are
gone, nearly none has another within the 1e-5 mm tolerance. Yet every vertex went through
eight hash probes, and every new cell allocated its own list.

## Decision

**`core-geometry` depends on `rayon`.** In a browser it runs on one thread, as it already
does for its peers (ADR 0177).

**Welding runs in three passes.** First, a parallel sort over bit patterns finds the exact
copies. Second, a parallel check against a sorted list of occupied cells sets aside every
distinct point with no other in the eight cells a match could be in, because such a point
merges with nothing. Third, the remaining points go through the grid one at a time, in
their order, with one map entry per cell chained through the representatives. The result
is exactly what one point at a time gave, and a test holds the two against each other.

**A hierarchy's halves are built on two threads above 16 384 faces,** then joined into the
same node array a single thread lays out, which a test also checks.

**The runs of the edge table and of the faces keyed by their corners are sorted a block
of 4 096 runs to a task.**

## Consequences

Benchmarks on the 200k-triangle sphere of `benches/`:

| Benchmark | Before | After | Change |
|---|---|---|---|
| weld | 32.0 ms | 19.0 ms | −41% |
| hierarchy | 29.4 ms | 10.5 ms | −64% |
| orient | 10.0 ms | 9.0 ms | −10% |
| `diagnose` | 8.6 ms | 7.6 ms | −12% |

Opening real files, with reading the file included:

| File | Before | After |
|---|---|---|
| 1.13M-triangle STL | 0.80 s | 0.40 s |
| 974k-triangle STL | 0.70 s | 0.35 s |
| 406k-triangle STL | 0.36 s | 0.16 s |

What is left is the text parsers: an 865k-triangle OBJ spends 0.64 s in reading alone.

Welding holds a sorted copy of the vertex bits and one of the occupied cells, about 30
bytes a vertex, until it returns.

## Alternatives considered

### A parallel weld that merges into whichever neighbour is nearest

It is simpler to parallelise and would not depend on order. Rejected because its result
would differ from today's on the rare crowded point, and every stored project, test and
fixture assumes today's.

### Sharing one edge table between orientation and diagnosis

It would save one of the two builds, about 25 ms on the largest file. Rejected for now: it
needs an API that hands a table from one call to the next, for less than either step's
remaining cost.

### The option that won, and what it costs

Three passes and two sorted copies replace one loop, and the identical result rests on the
argument that a point with no company in its probe cells cannot merge, which the test
checks only on the meshes it builds.

# 0315. Take `boolmesh` from our fork, threaded and deterministic

- **Status:** Accepted; the commit it pins is superseded by 0316
- **Date:** 2026-10-20

## Context

Exact booleans run on `boolmesh` 0.2.1 (ADR 0303) and sit behind the open floor, the
lattice, Combine, text and joints. On a ball of a million faces less a hundred prisms it
took 0.65 s to read the two solids and 0.92 s to cut them; the C++ original does the same in
a fifth to a seventh of that, and nearly all of the gap is threads: the port used one core
for all but five loops.

The same two meshes also came back triangulated differently from run to run, for two
reasons: `boolean45` walked hash maps, and its ear clipping broke ties by heap address. ADR
0303 lived with that by testing volumes rather than triangles.

Upstream has one author, who has merged none of the outside pull requests opened since
February 2026. The licence, MPL-2.0, lets a modified copy ship as long as its changed files
stay open.

## Decision

The workspace takes `boolmesh` from git, `github.com/vvvlladimir/boolmesh`, pinned to commit
`b6488b0` of its `parallel` branch, which is 0.2.1 plus two changes:

- **Deterministic.** The edge maps of `boolean45` are `BTreeMap`s and ear-clipping ties
  break by creation order, so a boolean depends on its input alone.
- **Threaded.** Reading a solid — weld, face sort, edge topology, normals, the collider's
  tree and boxes, the coplanar sort — and the boolean's collision queries, winding numbers,
  triangulation assembly and output buffers run on `rayon`, through one helper module.

Both hold one rule: the output is the same bit for bit on any number of threads and without
`rayon`, so the browser and the desktop agree. Each change is offered upstream, and the
workspace returns to crates.io once a release carries them.

## Consequences

At a million faces, reading falls from 0.65 s to 0.35 s and cutting from 0.92 s to 0.52 s;
in `core-volume`'s `boolean` bench, letters on a dense ball take 44% less time and an
opened floor 13% less. At four million faces the peak memory of one boolean falls from
6.5 GB to 5.6 GB, because edge topology sorts 16-byte rows instead of 32. The steps still on
one thread are the coplanar flood fill and the simplification, which are now the ceiling.

`cargo deny` allows this one git source. The fork is ours to rebase on each upstream
release, and its collider carries the workspace's only `unsafe` boolean code: parallel box
writes ordered by an atomic counter.

Tests may compare triangle lists of a boolean now, not only volumes.

Reopen when upstream releases the changes, or if keeping the fork level with it costs more
than the time it saves.

## Alternatives considered

### Wait for upstream

Costs nothing to keep, but nothing suggests the changes would land, and the run-to-run
difference stays until they do.

### Copy the crate into `crates/`

Puts 4,500 lines under the workspace's style and lint rules, which they were not written
for, and makes taking upstream releases a manual merge.

### The C++ original

The fastest, but it does not build for `wasm32-unknown-unknown`, and two engines would give
the desktop and the browser different meshes.

### The option that won, and what it costs

A fork to keep level with upstream, a git dependency, and an `unsafe` block we now own.

# 0314. Ready a solid for the engine on every core

- **Status:** Accepted
- **Date:** 2026-10-19

## Context

`Solid::new` welds a mesh on exact positions and pairs the faces round every edge before
`boolmesh` reads it (ADR 0303). On a sphere of a million faces that preparation was about a
third of the read, and all of it ran on one thread: a `HashMap` keyed by `SipHash` for the
weld, and two sorts of three tuples a face, `usize` wide, for the seams — 380 MB moved on
one core for a four-million-face model. An open floor, a lattice, Combine and a label all
read a model this way, and on a print-sized model the wait is seconds.

`core-csg` sat on `core-geometry` and `boolmesh` alone. Every other crate that does work
proportional to faces already runs it on `rayon` (architecture rule 7).

## Decision

`core-csg` depends on `rayon`. The edge uses are built and sorted with `par_iter` and
`par_sort_unstable`, as `u32` where the sort moves them, and the weld map is a `FastMap`.
Every sorted tuple is distinct or equal in full, so the order — and the solid — is the same
as before, bit for bit.

## Consequences

A sphere of a million faces is read in 0.66 s rather than 0.86 s, and one of four million in
4.3 s rather than 5.0 s, on an M1; the faces out are the same. What is left of the read is
the engine's own, which runs on one thread whatever its `rayon` feature says; that is the
next place to look, and it is not ours to change here.

A mesh past four billion vertices or faces no longer fits the sort's keys. No printer holds
one.

## Alternatives considered

### Leave the preparation on one thread

It is a third of the read, and the read is half of every exact boolean on a dense model.

### Parallelise the engine first

Where most of the time is, but it is another author's crate: a change there is a patch
offered upstream or a fork carried here, a decision of its own.

### The option that won, and what it costs

One more edge in the dependency graph, on a crate already in every build, and a thread pool
the engine's caller has to be ready to share: `core-csg` now uses whatever pool it is
called on, as `core-volume` does.

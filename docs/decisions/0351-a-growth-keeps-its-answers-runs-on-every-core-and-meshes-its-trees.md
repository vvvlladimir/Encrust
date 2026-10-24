# 0351. Keep a model's clearance answers, grow on every core, and mesh on the growth's worker

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

On a figurine of a million faces and 1 768 contacts, adding one support by hand took about two
seconds in the window. Each click regrew the whole forest: `columns` for every point, then
`grow`, which ran on one thread and spent 85 per cent of its 0.55 s in `clear`, the beam
sweep of ADR 0077 and 0341, often on beams it had swept in the run before. The trees were
then meshed in the frame they landed in (ADR 0340), and every ray of a beam's ring looked
through the model to its far side for a hit the sweep only reads within the beam.

## Decision

- **A model keeps what its sweeps answered.** `Clearances`, owned by `ModelSupports` and
  shared with each `Growth`, holds whether a beam, keyed by its exact bits, clears the
  surface at its placement. `Placed::remembering` makes `clear` read it before sweeping.
  Answers held against another surface address, face count or placement are dropped on
  first use; a new surface starts afresh. Sixteen locked shards of at most 32 768 answers
  each, a shard forgetting all when full, bound it to about 20 MB.
- **Growing runs on every core and gives the trees one thread gives.** What one tree, front,
  pair of trunks or merge asks is asked of all at once: standing the fronts, mending
  unsound trees in rounds the queue would take, bending blocked fronts, tying braces,
  standing small pillars on supports and meshing chunks of trees, each joined in order. The
  greedy merge judges the next 16 merges waiting with the one it needs, at once; a front
  never moves, so an answer holds until it is taken.
- **Only the axis of a beam looks past its end.** The ring rays stop at the beam's length;
  the axis still looks on to tell a beam that set off inside the solid.
- **The growth's worker meshes.** `Growth::grow` meshes the trees with the frozen ones and
  `take_grown` keeps those meshes unless a frozen tree changed meanwhile. A frozen tree
  carried by hand is still meshed in the frame.

This supersedes ADR 0340's meshing in the frame the trees land in.

## Consequences

On the crowded bench of nine 128 000-face balls and 664 tips, growing fell from 100 ms to
25 ms, growing again with one tip added from 103 ms to 14 ms, and meshing from 27 ms to
10 ms. On the figurine, growing and meshing fell from about 0.85 s to 0.15 s. Every result
is the one before, bit for bit: the kept answers and the speculative judgements are the
ones asked afresh. A ring ray that sets off in the solid and never leaves it inside the
beam now goes unseen, caught by the axis or the row of balls. Reopen if the answers outgrow
their bound on a real model, or if a frozen tree carried by hand stalls the frame.

## Alternatives considered

### Grow only the clicked support again

Faster still, but the forest would stop being a function of its points (ADR 0129).

### Judge every merge in the heap up front

Most of them die unjudged as their fronts merge: 64 at a time was slower than 16 on the bench.

### The option that won, and what it costs

A cache to keep coherent with the surface, by address as well as by reset, and a merge loop
that is harder to read than a plain heap.

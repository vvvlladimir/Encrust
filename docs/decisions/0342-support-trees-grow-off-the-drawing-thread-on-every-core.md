# 0342. Grow and mesh support trees off the drawing thread, on every core, keeping each model's clearance answers

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

A run places its contacts on a worker (ADR 0325), but the trees were grown from them, and
meshed, in the frame that drew the result. On a figurine of a million faces and 1 768
contacts that frame took seconds, and adding one support by hand regrew the whole forest in
it: `grow` ran on one thread and spent 85 per cent of its time in `clear`, the beam sweep of
ADR 0077 and 0337, often on beams it had swept in the run before, and every ray of a beam's
ring looked through the model to its far side for a hit the sweep only reads within the beam.

## Decision

- **Growing leaves the frame.** `ModelSupports::growth` takes what the trees grow from — the
  points, the blocked patch, the placement and the profiles — off the model as a `Growth`,
  which grows and meshes the trees, with the frozen ones, on a worker into a `Grown`;
  `take_grown` hands it back and keeps those meshes unless a frozen tree changed meanwhile.
  Until the trees land the supports drawn last stay on screen, and the window runs one
  growth per model at a time. Editing a picked support, and carrying a frozen tree by hand,
  still grow or mesh in the frame.
- **A revision says what is stale.** Every change to what the trees grow from — a point, the
  blocked patch, the surface, a freeze — stamps the supports with a number from one counter,
  never reused, so an undo cannot bring back a stamp a run in flight carries. Trees grown
  for another stamp are turned away and grown again.
- **Nothing reads supports that have not landed.** Slicing is refused and the preview waits
  while a model on the plate has supports pending; a project saved before its trees grew
  keeps its points and grows them when it is opened.
- **A model keeps what its sweeps answered.** `Clearances`, owned by `ModelSupports` and
  shared with each `Growth`, holds whether a beam, keyed by its exact bits, clears the
  surface at its placement; `Placed::remembering` makes `clear` read it before sweeping.
  Answers held against another surface address, face count or placement are dropped on first
  use. Sixteen locked shards of at most 32 768 answers each, a shard forgetting all when
  full, bound it to about 20 MB.
- **Growing runs on every core and gives the trees one thread gives.** What one tree, front,
  pair of trunks or merge asks is asked of all at once — standing the fronts, mending, bending,
  bracing, standing small pillars on supports, meshing chunks of trees — each joined in
  order. The greedy merge judges the next 16 merges waiting with the one it needs; a front
  never moves, so an answer holds until it is taken.
- **Only the axis of a beam looks past its end.** The ring rays stop at the beam's length;
  the axis still looks on to tell a beam that set off inside the solid.

## Consequences

The window keeps drawing while the trees grow. On the crowded bench of nine 128 000-face
balls and 664 tips, growing fell from 100 ms to 25 ms, growing again with one tip added from
103 ms to 14 ms, and meshing from 27 ms to 10 ms; on the figurine, growing and meshing fell
from about 0.85 s to 0.15 s. Every result is the one before, bit for bit. A ring ray that sets
off in the solid and never leaves it inside the beam goes unseen, caught by the axis or the
row of balls. For a moment after an edit what is drawn and what would be sliced differ, and a
support cannot be picked until it lands. Reopen if the answers outgrow their bound on a real
model, or a frozen tree carried by hand stalls the frame.

## Alternatives considered

### Keep growing in the frame and make it faster

No speed keeps a frame under 16 ms on a plate of thousands while each support added grows it.

### Grow only the clicked support again

Faster still, but the forest would stop being a function of its points (ADR 0129).

### Judge every merge in the heap up front

Most of them die unjudged as their fronts merge: 64 at a time was slower than 16 on the bench.

### The option that won, and what it costs

The Slice button and the preview wait on a growth, a cache to keep coherent with the
surface, by address as well as by reset, and a merge loop harder to read than a plain heap.

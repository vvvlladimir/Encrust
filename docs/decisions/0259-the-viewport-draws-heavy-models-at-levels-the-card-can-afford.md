# 0259. Draw heavy models at levels the card can afford, chosen by what they show on screen

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

A plate of many high-polygon models is tens of millions of faces a frame. Measured on the
card, eighteen million cost 22–29 ms on one machine, so orbiting ran at 15 frames a second
and a pointer passing over a resting plate did as well. A slow machine is many times
worse; a fast one draws the same plate well inside a frame. The viewport has to keep up on
both without simplifying what the fast one can draw.

A single light copy built by vertex clustering, swapped in while the camera moved, kept
the frame up but popped visibly both ways, looked blobby, and was drawn for every model or
none: it had no measure of what it gave up, so it could not choose which model to lighten
or be trusted at rest. Frame intervals are no measure of the card either: they hold vsync,
the window's own pass and a hand pausing on the pointer.

## Decision

- **A chain of levels per heavy mesh** (`render/lod/chain.rs`), each half the one before,
  built with `meshoptimizer-rs` 0.2 on a worker: edge collapse with an absolute error, and
  its grid simplifier where collapse stalls. Each level carries an error in mm, the larger
  of the simplifier's summed along the chain and a sampled distance from its faces to the
  mesh.
- **The card's own time** (`render/gpu/timer.rs`): the scene pass carries timestamp
  queries where the adapter offers `TIMESTAMP_QUERY`, read back without waiting. A
  least-squares line through the passes gives a fixed cost and a cost a face; the budget
  is the faces left in 12 ms moving and 30 ms at rest. Without timestamps, runs of slow
  frames cut the moving budget.
- **Levels chosen by pixels** (`render/lod/allot.rs`): a step's error times the pixels a
  millimetre spans at the model's nearest point; the step showing least goes first, any
  while the camera moves, only those under half a pixel at rest. After the camera stops
  detail returns a level a frame and never coarsens.

## Consequences

- A fast card draws every model whole, moving or not; a slow one keeps 60 Hz in motion and
  gives up at rest only what is under half a pixel.
- Each heavy mesh holds about as many faces again in its levels, in memory and on the card
  while they are drawn; a million faces cost a second of a worker's time once.
- The web build keeps all of it on WebGPU; on WebGL it falls back to the slow-frame budget
  and draws resting frames whole.
- `meshoptimizer-rs` is young and has one author. Its API follows meshoptimizer's, so the
  bindings to the original library could replace it in one file. Reopen if it falls behind
  the original or a level shows holes the original would not make.

## Alternatives considered

### Vertex clustering of our own, at several levels

No dependency, and its error is bounded by the cell. It loses detail far faster than edge
collapse for the faces it keeps, so the same frame would look worse in motion and the rest
rule would allow less.

### The meshoptimizer library itself, through `meshopt`

The reference implementation and mature, but C++ built by `cc`, which does not build for
`wasm32-unknown-unknown` without a sysroot the web build does not have.

### Lowering the render resolution while the camera moves

Cheapens the pixels, not the faces, and the faces are the cost here: a light pass of two
million faces already costs most of a frame's fixed share.

### The option that won, and what it costs

A new dependency on a young crate, a second geometry per heavy mesh, a GPU feature asked
for at start-up, and a per-frame choice whose inputs — the line, the pixels — have to be
right for the picture to be. A bad estimate shows as a model simplified that need not be.

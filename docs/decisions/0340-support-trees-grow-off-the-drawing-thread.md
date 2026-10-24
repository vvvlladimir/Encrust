# 0340. Grow support trees off the drawing thread, and look round a pad only as far as it reaches

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

A run places its contacts on a worker (ADR 0325), but the trees were grown from them in the
frame that drew the result: `ModelSupports::refresh_on` dropped every point to its landing
and merged the tips into trunks there and then. On a figurine of a million faces that frame
took 0.6 s for 600 supports and 1.6 s for 1 450, and every **Grow again** added to it, so the
window froze for seconds. Nearly all of it is rays through the model's hierarchy, and most of
those are a pad looking round for the model: 16 sectors, three looks each, at three heights,
every ray running the whole way through the model whatever it hit.

## Decision

- **Growing leaves the frame.** `ModelSupports::growth` takes what the trees grow from — the
  points, the blocked patch, the placement and the profiles — off the model as a `Growth`,
  which grows on a worker into a `Grown`; `take_grown` hands it back. Meshing the trees,
  some milliseconds, stays in the frame. Until the trees land the supports drawn last stay
  on screen, and the window runs one growth per model at a time.
- **A revision says what is stale.** Every change to what the trees grow from — a point, the
  blocked patch, the surface, a freeze — stamps the supports with a number taken from one
  counter, never reused, so an undo cannot bring back a stamp a run in flight carries.
  Trees grown for another stamp are turned away and grown again.
- **Nothing reads supports that have not landed.** Slicing is refused and the preview waits
  while a model on the plate has supports pending; a project saved before its trees grew
  keeps its points and grows them when it is opened.
- **A pad looks only as far as it reaches.** Each look is cut off at the pad's radius plus
  its clearance (`Bvh::raycast_within`). Whether a height sets off inside the model is asked
  once per height along a ray of any length, so a pad deep inside a wide part still has no
  room.

## Consequences

The window keeps drawing while the trees grow; they appear a second later on a dense model
rather than the window stopping for that second. Editing a picked support still grows in the
frame, since a support that vanishes under the cursor cannot be aimed. On a ball standing on
the plate, `grow` takes 70 ms rather than 102 ms (`benches/branch.rs`); on a model lifted
clear of its pads nothing changes. Growing is still one thread per model and grows again
from nothing on every change: if a dense plate keeps its supports pending too long to work
with, growing in parallel or only around what changed is the next step.

## Alternatives considered

### Keep growing in the frame and make it faster

Halving the rays halves the freeze but leaves it growing with every support added; no speed
keeps a frame under 16 ms on a plate of thousands.

### Grow everything on the worker that places

Covers a run, but not an edit, a profile retuned or a model moved, all of which grow again.

### The option that won, and what it costs

For a second after an edit what is drawn and what would be sliced differ, so the Slice
button and the preview have to wait on it, and a support cannot be picked until it lands.
Each pad pays three unbounded rays where the first of its looks used to answer.

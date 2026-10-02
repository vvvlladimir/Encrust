# 0029. Find overhangs on the slice stack, not on the mesh

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

Step 7b has to decide, without being told, where a model needs holding up. The familiar
formulation is the overhang angle: a face steeper than some threshold against the build
direction gets a support. That is the FDM formulation, where an extruded filament has to
bridge air it is laid over.

MSLA fails differently. A layer cures against the film at the bottom of the vat and is
then peeled off it. What tears is *area that appeared from nowhere*: a cross-section with
nothing cured under it has only the film's adhesion holding it, and the peel takes it away
or drags it out of place. A wall at 20 degrees off vertical has no unsupported area at all
and prints happily; a flat plate one layer thick, held on a single pin, has no steep face
anywhere and fails every time. The FDM slicers replaced their angle-based SLA generators with
a layer-linking one for this reason.

`core-supports` had no access to layers. It was a peer of `core-slicer` in the dependency
graph: both sat directly on `core-geometry`, and neither referenced the other.

The run also has to be interruptible. Finding the overhangs means cutting the plate again
at the print's own layer height, which is the same work the Slice button does — hundreds
of milliseconds to seconds — and cannot happen inside a frame.

## Decision

Automatic placement reads a `Sliced` stack, layer by layer, and compares each layer's area
with the layer below it:

- a part that shares no area with the layer below is an **island**, and all of it needs
  support;
- a part that does overlap, but reaches more than `SELF_SUPPORTED_WIDTH_MM` (1.5 mm) past
  the layer below, is a **peninsula**, and the coast it reaches out with needs support
  where that coast is at least `PENINSULA_MIN_WIDTH_MM` (2 mm) wide;
- a layer whose underside sits on the build plate needs nothing, whatever is under it.

Supports already standing hold up what is near them, over a radius that grows with how far
above them a layer is — the `SUPPORT_CURVE`, scaled by the profile's `density`. That area
is subtracted before anything is sampled, which is what stops a ledge from collecting a
fresh row of supports on every layer of its own height, and what makes a hand-placed
support suppress the automatic ones around it.

`core-supports` therefore depends on `core-slicer`. It stops being a peer of it and sits
one layer above, in the same relation `core-raster` already has to `core-slicer`.

The public entry point is

```rust
pub fn generate_supports(
    stack: &Sliced,
    layer_height_mm: Scalar,
    profile: &SupportProfile,
    seeds: &[Vec3],
    progress: &mut dyn FnMut(usize) -> bool,
) -> Vec<Vec3>
```

It takes a stack rather than a mesh: the crate does not slice, and the caller already owns
a slicing pipeline and the thread pool to run it on. `progress` reports the layer and stops
the run when it returns `false`, which is how cancellation reaches the loop without
`core-supports` knowing what a job is.

In the window, `SupportJob` runs one `generate_supports` per visible model on a worker
thread, with a progress bar and a cancel button, the same shape as `SliceJob`
(`docs/decisions/0018`). Each model is cut on its own, so a contact belongs to the model it
was found under and is stored in that model's own space, per `docs/decisions/0028`.

## Consequences

- What the generator sees is what the printer will expose. A change in layer height changes
  the placement, which is correct: the layer height is what decides how much new area
  appears at once.
- The run is deterministic. The same model at the same height and density produces the same
  points, every time, which is what makes a regeneration predictable and a test possible.
- Supports already placed, by hand or by an earlier run, are handed in as seeds and are
  never moved or returned. Placement is additive.
- The cost is a second slicing run. On the shipped benchmark — four legs, two tables,
  1200 layers — detection and sampling take 20.6 ms on top of the slicing itself. A layer
  that needs nothing costs one offset and one boolean per part; only a layer that carries
  an island or a coast pays for a coverage union.
- Coverage is an area on the plate, not a volume. A support does not know whether the
  material above it is the same solid it carries, so a part that passes over an unrelated
  column within the curve's reach can be left with one support fewer than it wants. The
  signal to reopen this is a print that fails over a part standing beside a tall support;
  the fix is to carry the linked parts up the stack.
- Branching (step 7c) changes nothing here. It consumes the same points.

## Alternatives considered

### Overhang angle on the mesh faces

Needs no slicing and no polygon library, and would have been a day's work. It answers the
wrong question for resin: it fires on steep walls that print fine and stays silent on flat
islands that do not. Every serious MSLA slicer has moved off it.

### Detect on the mesh, but by finding local minima of the surface

Cheap, and it does find islands — the lowest point of each disconnected piece of surface.
It cannot find peninsulas at all, and on a tessellated model the minima land on whichever
vertex the mesher happened to put lowest rather than in the middle of the area that needs
holding.

### Slice inside `core-supports`

Would have kept `generate_supports` to one argument and hidden the layer height. It would
also have put a slicing run, and the thread pool it needs, inside a crate that is supposed
to answer questions rather than run pipelines — and the window would then slice the same
model twice, once for the preview and once for placement, with no way to share the result.

### The option that won, and what it costs

Reading the stack costs the dependency on `core-slicer`, which is a real narrowing: from
here on a support generator cannot be used on a mesh that has not been cut, and anything
that wants supports has to own a slicer. It also costs the second slicing run, and ties the
answer to the layer height, so changing the height silently invalidates the supports on the
plate — nothing warns about that yet. Both were judged cheaper than a generator that gives
the wrong answer on the geometry resin actually fails on.

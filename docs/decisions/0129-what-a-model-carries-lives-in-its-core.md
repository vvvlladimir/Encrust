# 0129. What a model carries lives in the core that makes it

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

Each object in the window carried two containers of its own: `ObjectHollow` (blockers,
drain holes, channels, the points of a channel being laid out, the shell a run built) and
`ObjectSupports` (support points, painted patches, frozen trees, the trees and meshes built
from them). Between them, about 900 lines that never touch egui — turning a click in
plate coordinates into the model's own space, deciding when a shell is stale, re-cutting
holes against a wall — lived in `encrust-app`, where no other front end and no core test
could reach them. The architecture audit (F7) named this as the reason the window grows
faster than the cores.

`ObjectHollow` also held the pockets the last drainage check found. Those are
`core_supports::Trapped`, found by reading the slice stack, and `core-volume` may not
depend on `core-supports` or `core-slicer`.

## Decision

`ObjectSupports` moves to `core-supports` as `ModelSupports`, unchanged but for its name:
everything it needs is already in that crate or below it.

`ObjectHollow` moves to `core-volume` as `ModelHollow`, without the pockets. A finished
run is handed over as `core_volume::Shell` rather than the window's `Shelled`, and a hole
is sized by `HoleSize` rather than the Drain tool. The pockets and their balls stay in the
window as `Traps`, a field of the scene object beside `hollow`, and are drawn as a second
marker mesh. `core_volume::markers` meshes both, so they look the same.

`Preview` stays in the window: it holds egui textures and the jobs that fill them, and is
a view of the stack rather than something the model carries.

## Consequences

Both containers are now tested in their cores (31 tests moved), and a second front end
gets a model's supports and cuts without the window. The scene object has one more field,
`traps`, which is the honest shape: the drainage check and the cuts it prompts are two
different crates' questions. A drainage check no longer shares a marker mesh with the
blockers, so the viewport draws one more small mesh per checked model.

## Alternatives considered

### Move `Trapped` down so `ModelHollow` can keep the pockets

The only crate both `core-supports` and `core-volume` see is `core-geometry`, which is
geometry, not drainage. Putting a drainage result there to save one field on the scene
object bends the crate that has to stay a leaf.

### Leave both in the window until step 19

What the audit first proposed. It keeps the growth where it is, and the move does not
depend on anything step 19 will redesign: these are data and their bookkeeping, not tools.

### The option that won, and what it costs

`ModelHollow` and `Traps` together are what `ObjectHollow` was, so the one question "what
does this model's drainage look like" now reads two fields.

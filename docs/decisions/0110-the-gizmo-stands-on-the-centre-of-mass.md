# 0110. The gizmo stands on the model's centre of mass

- **Status:** Accepted
- **Date:** 2026-09-27

## Context

`transform-gizmo-egui` draws its handles at the translation of the transform it is given,
and rotates and scales that transform about the same point. The viewport handed it
`SceneObject::transform` directly, so the handles stood on the model's own origin —
wherever the STL's author left it, which for a great many files is a corner of the
bounding box, or a point nowhere near the mesh at all. Rotating then swung the model
around that arbitrary point instead of turning it in place.

## Decision

The viewport hands the gizmo a *pivot* transform per selected object: the object's
rotation and scale, with the translation replaced by its centre of mass in plate
coordinates. What comes back is put through `SceneObject::settle`, which keeps the
returned rotation and scale and solves the translation so the centre of mass lands on the
returned pivot.

The inspector's transform fields drag the same pivot, so the three numbers it prints are
the point the handles are on, and typing into them turns and stretches the model about
that point exactly as dragging does.

The centre of mass is the volume centroid of the solid, `core_geometry::center_of_mass`,
not the middle of the bounding box: a model with mass at one end has its handles where the
mass is. A mesh enclosing no volume gives the middle of its bounding box instead.

It is measured once per mesh in `Imported::new`, beside the `Bvh`, and stored on the
object, because the gizmo asks for it every frame and the sum is over every face.

## Consequences

- Dragging behaves exactly as before; rotating and scaling now happen about the model
  rather than about its file's origin, which is what every other slicer does.
- `Imported` and `SceneObject` carry one more field that has to stay in step with `mesh`.
  `Imported::new` is the only way a mesh reaches an object, so it does.
- Restoring a project recomputes it: the `.encrust` manifest stores the mesh, not derived
  numbers, so `project/state.rs` measures it alongside the hierarchies it already builds
  in parallel.
- The position in the inspector is the centre of mass, not the model's origin and not the
  middle of its bounding box, which is what other slicers show. Nothing prints the origin any
  more; it is an implementation detail of the file the model came from.
- The panel writes back only on the frames a field changed. Settling every frame would
  round-trip the transform through the centre of mass and let the error accumulate.

## Alternatives considered

### Put the handles on the bounding box centre

Cheaper, and needs no new geometry. Rejected because it is wrong for anything that is not
roughly symmetric: a figure with a wide base and a thin spire gets handles halfway up the
spire, in empty air.

### Re-origin the mesh on import, so the transform's translation is already the centre

Makes the pivot free and the inspector honest. Rejected because it rewrites vertices the
user gave us: a model's coordinates would no longer match the file it came from, and every
saved project would have to record the shift to stay reproducible.

### Move the handles but leave the inspector on `transform.translation`

Half the change, and the smaller diff. Rejected because the two would then disagree on
screen: dragging a handle would move numbers that do not match where the handle is.

### The option that won, and what it costs

A volume centroid can sit outside the solid — inside a ring, between the two halves of a
fork — so the handles are sometimes on nothing and the position field names a point in
empty air. It is also one derived field to keep in step with the mesh.

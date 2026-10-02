# 0075. A cut is its own operation, not part of hollowing

- **Status:** Accepted
- **Date:** 2026-09-23

## Context

Drain holes and channels were fields of `HollowSettings`, and `hollow` appended their
bodies to the shell it built. That tied a cut to a cavity three ways: a hole did nothing
until a hollow run was started, placing one made the shell stale so the whole field had to
be built again, and a solid model could not be drilled at all.

None of that follows from the geometry. A cut is a closed body wound inward appended to
whatever it cuts (ADR 0071); the fill rule subtracts it wherever it lands, cavity or no
cavity. The only thing the cavity contributes is a wall thickness, and only to a hole that
would otherwise stop inside it.

## Decision

`drill` returns the bodies already wound inward, and whoever is about to slice or draw a
model appends them: `merge_visible` and the drainage check in the window, `hollowing::cut`
in the CLI. `HollowSettings` carries blockers and nothing else about cuts.

Depth is the depth asked for. `pierce` deepens a hole to `lift + thickness + 2 voxels`
against the cavity a model was hollowed to, and is applied where that cavity is known:
`ObjectHollow::recut` on every change to a cut or to the shell, and once in the CLI after a
hollow run. A solid model keeps the depth that was typed, so a click cannot tunnel through
a part nobody asked to open.

## Consequences

A hole opens on the click that places it, in the viewport and in the layer preview alike,
and the model no longer has to be hollowed twice to see one. Nothing draws a marker tube
any more: a cut is geometry from the first frame, so `ObjectHollow` meshes the bodies once
per change and hands the same list to the renderer and to the slicer.

Hollowing a drilled model re-cuts its holes through the new wall, and clearing the cavity
puts them back to the depth they were asked at. The cut bodies are meshed twice over on
that change — a few hundred triangles, against a field that takes seconds.

Two places now append the same bodies, the merge and the drainage check, rather than one
`hollow` doing it for everybody. A third caller that slices a model and forgets them would
print a model with no way out, which is what `a_hole_placed_on_a_model_is_open_in_the_layers_that_cross_it`
and `a_drain_hole_is_cut_into_a_solid_model_too` are there to catch.

## Alternatives considered

### Keep the cut in `hollow` and re-run it on every change

What we had. A field over a real model is seconds of work, so every hole cost a rebuild of
geometry that the hole does not change, and a solid model stayed undrillable.

### Bake the cut into the shell mesh once, at the end of the hollow run

Cheaper to slice, and wrong the moment a hole moves: the shell would have to be rebuilt to
take a cut back out, which is the problem this decision removes.

### The option that won, and what it costs

The mesh a model is drawn from and the mesh it is sliced from are now assembled from two
pieces by each caller instead of being one object. The pieces are small and the assembly is
four lines, but there is no longer a single value that *is* the drilled model, and a caller
that wants one has to put it together.

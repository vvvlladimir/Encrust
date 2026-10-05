# 0073. Subtract a drain per fragment in the viewport

- **Status:** Superseded by 0188 (a channel's wall only)
- **Date:** 2026-09-23

## Context

Everything this workspace subtracts is a closed body appended wound inward, and the fill
rule does the arithmetic when the layer is rasterised (ADR 0059, 0071). The viewport does
no such arithmetic: it draws every triangle it is given, so a drilled model still shows an
unbroken surface with a tube standing in it. A hole reads as a boss, and a user checking
where their holes went cannot see whether any of them went through.

Cutting the mesh for real is not on the table. The outer surface is the imported model,
never remeshed (ADR 0059), and the workspace has no polygon boolean.

What the viewport does have is the holes themselves: a handful of analytic cylinders and
cones per model, and a channel is a chain of capsules around the same segment test.

## Decision

The fragment shader subtracts them. `Globals` carries up to `MAX_CUTS` drains in plate
millimetres — two `vec4`s each, an end and its radius — and both the model pass and the
section-crossing pass discard a fragment standing inside any of them.

The cuts handed over are the tubes `ObjectHollow::cut` holds — every hole and channel on
the model, deepened through the wall wherever there is one (ADR 0075), and not the depth
they were asked at, or the hole is drawn plugged at the bottom.

What shows through a hole is the inside of the model, so the inside has to look like one:
every back face is washed with the section colour, not only while the plate is cut. An
unwashed interior is lit like the surface around it and a hole reads as a flat disc.

A hole with nothing drawn inside it is a window, not a hole, so the viewport also draws
the bore. `bores` meshes the wall of every cut and the floor of every hole that ends in
material, and clips it to the model as it stands: each sector of each tube is cast along
the mesh and kept only over the stretches that run through material, and only for the wall
a cavity left. A wall that ignored either would stand out of the model as a boss or cross
its cavity as a rod. The test is narrowed by three hundredths of the radius so that wall —
a prism inscribed in the radius — survives it. Nothing else sees that mesh: it is drawn,
never sliced and never counted.

The test is the frustum the cut was meshed as, not a capsule: rounded at the ends that
open, so a surface curving away under a mouth goes with it, and flat at a hole's floor,
which is drawn. A negative radius at the far end is what marks an end that opens.

## Consequences

A drilled model looks drilled, cut and uncut holes are told apart on sight, and the section
cap opens over a hole because the counting pass discards the same fragments.

Cost is a loop over the cuts per fragment, over a count that is a plate's holes — tens, not
thousands. Past `MAX_CUTS` the later holes are drawn uncut; nothing about the print
changes, only the picture, and the number can be raised for the cost of the uniform.

This is display-only, and now there are two places that know how a hole is subtracted: the
fill rule and the shader. They agree by having the same shape handed to both, but a change
to how a hole is meshed has to be made in both. The signal to reopen is a subtractive body
that is not a capsule — a cut plane, a boolean against another model — because that one
will not fit in this uniform.

## Alternatives considered

### Cut the mesh with a real boolean

Exact, and what the viewport would ideally draw. Rejected on the standing ground that the
workspace has no polygon boolean and the model is never remeshed: a boolean would have to
mesh the whole shell again on every change to a hole.

### Depth-peeled CSG in the render pass

The general solution, and it would cover every subtractive body rather than capsules alone.
Rejected as several passes and a stencil protocol for a case that one distance test answers.

### The option that won, and what it costs

The viewport now lies in the other direction: it draws a hole where the mesh has none, and
it is the slicer that makes that true. A bug that stopped holes reaching the fill rule
would still look right on screen, and the layer preview is what would catch it.

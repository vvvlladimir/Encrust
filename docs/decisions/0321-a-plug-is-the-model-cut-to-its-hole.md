# 0321. A plug is the model cut down to its hole

- **Status:** Accepted; amended by 0346 for a hole drilled before the cavity
- **Date:** 2026-10-21

## Context

ADR 0313 made a plug by sampling its outer face: points on rings over the mouth, each
dropped onto the model by a ray, and a point no ray finds read as the mouth's own plane.
That holds on a broad face. On anything narrower than the hole — the tip of a cone, a pin,
an edge — most rays miss, and the plug comes out as the hole's full cylinder where the
hole took a point, so it cannot close what it was made for. The plug has to be the
material the hole took, exactly.

## Decision

`plugs` intersects the model with the hole's body through `core-csg`: a closed frustum, the
hole's outline moved in by the gap, from the lifted mouth to a flat inner end. The model is
read as a solid once for all its holes, its shells unioned first as `combine` reads them,
and each plug is that solid's meeting with one body, less the holes drilled before it that
meet it, so its outer face is the model's own surface and a hole drilled over another has
that hole through its plug. A model the engine refuses is patched for it first: duplicates
dropped, faces turned outward, holes filled, and the faces round any edge still not walked
once each way cut out and filled over, which a plug minds only near its hole. One refused
even then keeps the sampled face of ADR 0313, which this record supersedes for the plug's
shape only; `plugs` counts such plugs, and both front ends say so. `combine` and
`combine_exact` patch a refused model the same way before ADR 0304's field takes it.

Reading the model is most of the cost, and a change of gap or a hole moved nearby does not
change it, so `plugs` reads the model only once its holes are found sound, crops it through
the engine to a box 5 mm past each body a hole could leave at no gap, hands the caller that
piece as a `PlugSource` for the next call, and cuts the holes' plugs from it in parallel; a
hole past every box has the model read again. A read that would keep the whole model, every
box holding it or the crop failing, is not handed back. The window cuts plugs on a worker
and lays them into the edit that asked for them, so one undo takes back hole and plug, and
sets each down at the nearest free spot on the plate, `core_plate::nearest_free`; how is in
docs/design/hollowing.md.

## Consequences

A plug over a cone, a pin or an edge is the piece the hole took, flush to the face at
every point rather than at its samples. Reading the model costs the most, about 0.4 s for one
plug on 1.5 million faces, and more where it must be patched first; cropping makes the first
read dearer and every later cut several times cheaper. The window does not pause for any of
it, and the old plugs stand until the new ones land. The read still peaks at some 590 MB on
1.5 million faces, which the allocator keeps for reuse; lowering that peak is the engine's
own representation, a separate decision.

## Alternatives considered

### Cutting the model down plane by plane

`core_geometry::cut` once a side of the body, a box round the hole first. Each cut is
exact alone, but a chain of thirty of them, the mouth's plane a twentieth of a millimetre
off the surface, left a seam open in nearly every cap on clean models, and the chain fell
back to sampling for all but a few of 120 holes tried.

### More rings, and the rim read off the surface

Sampling finer narrows the error but never closes it: a face no ray hits still reads
as the mouth plane, so a cone tip stays a cylinder.

### The option that won, and what it costs

A pass of the whole model through the engine the first time its plugs are cut in the Drain
tool, again for a hole past the piece kept, and whenever they are cut from elsewhere; a
piece of the model held read while the tool is open; plugs that trail their holes by a
cut's length, and by a second one where the holes moved during the first; a patch that can move the surface where it was torn, should the hole
fall there; and two ways of making a plug where there was one, the sampled one used only
on a model no patch makes readable.

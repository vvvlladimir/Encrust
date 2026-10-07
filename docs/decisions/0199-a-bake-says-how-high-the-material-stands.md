# 0199. A bake says how high the material stands, and the stack stops there

- **Status:** Accepted
- **Date:** 2026-10-07

## Context

A stack covers the baked mesh's own box, from the plate to its top (ADR 0187). Everything
this workspace subtracts is a closed body appended wound inward (ADR 0071), and a cut has to
reach past the surface it pierces or a film of resin is left over its mouth. A drain hole
drilled into a vertical face near the top of a model therefore stands up to its own radius
over everything that prints.

The stack followed it. A 3 mm hole on the top corner of a 15.5 mm bracket planned 338 layers
instead of 310: twenty-eight layers of blank mask, which the printer still peels, and a file
claiming 16.9 mm of height. The masks under them were identical, so nothing was wrong with
what printed — only with how much of it there was.

Nothing in the mesh says which of its shells only subtract. The caller that merged it knows:
a `Model` already carries its cuts apart from its geometry, because the window places them
on the model as it was imported (ADR 0129).

## Decision

`bake` answers a `Baked`: the merged mesh, and `ceiling_mm` — the top of the material,
measured over the models and the supports with the cut bodies left out. `cut` takes that
`Baked` and plans no layer above the ceiling, through `Windows::under`,
`adaptive_plan_under` and `layer_heights_under`, which clamp the top the way `on_the_plate`
already clamps the bottom. `Baked::of` is a mesh that carries nothing but material.

The command line therefore keeps its cut bodies in `Model::cuts` rather than appending them
to the model, as the window already did. `stand_under` answers with the supports instead of
appending them, and finds its contacts on the model and its cuts together, which is what it
did when they shared a mesh.

## Consequences

A hole near the top of a model costs no layer. Every front end gets this from `cut`, so the
window, the browser and the command line agree, and the drainage check — which cuts a model
of its own — stops at the shell rather than at the hole standing over it.

A caller that cuts a mesh it built itself has to say so with `Baked::of`, and one that passes
a mesh with cut bodies already in it gets the old behaviour rather than an error. The signal
to reopen is a second kind of body that only subtracts and does not travel in `Model::cuts`.

The command line now holds the model's mesh and its cuts apart until the bake, which is one
copy of the cut bodies — tens of triangles — and one copy of the model while supports are
being placed.

## Alternatives considered

### Work the material's box out of the mesh

Split the baked mesh into shells and take the box of the outward-wound ones. It needs no
caller to tell it anything, and it is right by construction. Rejected as a connected-shell
pass over the whole plate on every cut, to recover a fact the caller had in its hand.

### Clip the cut bodies to the model's box

Trim the tube where it leaves the model, so the baked mesh is never taller than the material.
Rejected on the standing ground of ADR 0071: a cut trimmed against the model leaves slivers
of material across its mouth, which is the trap the whole positive-winding rule was adopted
to avoid.

### Drop the empty layers after the stack is cut

Plan as before and throw away the trailing layers with nothing in them. It needs the stack
to exist before its own height is known, and the height is in the header a streaming write
lays down first (ADR 0012).

### The option that won, and what it costs

Two ways to ask for a stack where there was one, and a `Baked` that a caller can build with
the wrong ceiling. The command line's support placement moved from "the model with its cuts
in it" to "the model and its cuts, merged for the pass that finds contacts" — the same
answer, by a path that has to be kept the same on purpose.

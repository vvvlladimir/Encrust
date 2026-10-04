# 0059. A hollow is the model with its cavity appended, not welded

- **Status:** Superseded by 0185 (the space the wall is measured in only)
- **Date:** 2026-09-21

## Context

A hollowed model is a solid with a hole in it. The usual way to produce one is a mesh
boolean: subtract the inward offset from the original and weld the result into one
watertight surface. The workspace has no mesh boolean engine, and adding one is a large
dependency with a long tail of degenerate cases.

It also has no need of one. Supports have been appended beside the model rather than
welded to it since step 7d, because the rasteriser fills by the non-zero winding rule
(ADR 0041, ADR 0043): overlapping solids union for free, and a surface wound the other
way subtracts for free. A cavity is exactly the second case.

There is a second reason not to weld. The cavity comes out of marching cubes, which
resamples whatever it touches onto the lattice. A boolean would put the *outside* of the
model through that lattice too, and a face sampled at 0.2 mm is a face that has lost its
detail. Every other slicer hollows this way and every one of them softens the model doing
it.

## Decision

`hollow` returns the original mesh with two things appended to it:

- the cavity surface, extracted from the field banded at `d = -t` and wound inward;
- whatever fills the cavity, wound outward — a lattice of overlapping boxes, by ADR 0060.

Nothing is welded, nothing is deduplicated, and the model's own vertices and faces come
through byte for byte, first in the buffer. `External` is the same trick read the other
way: the grown surface is the outside and the model itself is appended as the cavity.

The winding then does the arithmetic. Inside the wall the count is `+1`; inside the
cavity it is `+1 - 1 = 0`; inside a strut of infill standing in the cavity it is
`+1 - 1 + 1 = 1`. The rasteriser fills where the count is not zero, so a shell, its
cavity and its infill come out of the same stack of contours with no boolean anywhere.

Because the count outside the model is zero and a cavity contour alone would make it
`-1` — material, in a place nothing surrounds — neither the cavity nor anything standing
in it may reach outside the solid. `BottomThrough` therefore clips its downward prism against the model's own field
rather than against a plane: the cavity's floor ends up exactly on the model's underside,
coincident with it, which cancels rather than adds.

## Consequences

Hollowing is cheap and lossless on the outside: the cost is the cavity's own surface
area, and the model's triangles are untouched whatever the lattice is. A 500k-triangle
model hollows in the time its cavity takes, not the time its surface would take to
remesh.

The result is not a manifold mesh, and `diagnose` will say so. That is already true of
every supported plate, and nothing downstream of the rasteriser asks. But it does mean a
hollowed mesh cannot be handed back to `build` to make a field of — the winding number
would read the cavity as a second shell. Step 8e's drain holes therefore cut the
*cavity's field* before it is meshed, not the hollowed mesh afterwards.

The window keeps the hollowed mesh per object, in the model's own space, and only slices
with it. The wall is measured in that space, so a model rescaled after it was hollowed
carries a wall of the wrong thickness; the tool says so and asks to be run again rather
than silently rebuilding, because rebuilding is a second of work.

The signal to reopen this: an export that has to hand a watertight mesh to another
program — an STL of the hollowed part, say — would need a weld, and would need it to be
a real boolean rather than a vertex merge.

## Alternatives considered

### A mesh boolean

The answer a modelling package would give, and the only one that yields a watertight
result. It costs a dependency the workspace has avoided twice already (ADR 0032,
ADR 0040), it remeshes the outside of the model, and it is where every hollowing bug in
every other slicer lives. Rejected for all three.

### Return the cavity as a field and let the caller do the CSG

Keeps everything in the field domain and would make step 8e's holes a `difference` on the
result. It cannot produce the outer surface without extracting the model's own field,
which is the remeshing this decision exists to avoid.

### The option that won, and what it costs

An appended cavity is a lie told to the rasteriser that happens to be true. It only works
because the fill rule is non-zero winding; a slicer that filled by the even-odd rule would
read the infill struts as holes in the cavity, and would read one box of a lattice
overlapping the next as a hole between them. That rule is now load-bearing in four places
(supports, the base, the cavity and the lattice inside it) and changing it would break all
of them at once — `core-raster`'s own tests pin it, which is the guard.

It also means a "hollowed model" is not a thing that can be inspected on its own. The only
way to ask whether the cavity came out right is to slice it, which is why the test that
proves it lives in `encrust-cli` and runs the whole pipeline rather than in `core-volume`.

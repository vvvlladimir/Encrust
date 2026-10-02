# 0123. A face names its own image

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

ADR 0115 gave a model one texture, on the grounds that one is what a relief is pressed
from. A model is not built that way. A statue downloaded as one OBJ is bronze, marble,
brick and paving, four images over six materials; a game model is hull and tracks. Reading
only the first material's image textured the whole model from whichever part happened to be
listed first, and the 3MF loader refused any build whose groups named two images at all.

## Decision

`UvMap` holds a `Mapping` per face — the three corners and the index of the image they land
on — and `Loaded` carries `textures: Vec<Texture>` in the order the map indexes them.
`press` takes `&[Heightmap]` and reads each face from the image its own face names.

The OBJ loader reads every material's `map_Kd` once, sharing one entry between materials
that name the same file, and a `usemtl` already splits the file into models, so a model's
material is what says which image its faces take. The 3MF loader gives each
`texture2dgroup` the index of the image its `texid` resolves to; the rule that a second
image unmaps the file is gone with it.

An index has to name an image that exists: a group whose part is missing from the package,
or a material whose file is not beside the model, leaves its faces unmapped, and `press`
refuses a map naming more images than it was handed.

The viewport draws a model that still carries a map with its images washed over it while
the Relief tool is open, as the heights they are — white proud, black flat — so where the
texture lands can be seen before it is pressed. The images go to the card as the layers of
one array texture, each resampled to 512 square, and a face names its layer.

## Consequences

A model textured by material presses each part from its own image, which is what makes the
feature usable on a downloaded asset rather than on a test cube.

The textured pass is its own pipeline over its own vertices — position, normal, coordinate
and layer — so a model drawn that way costs a second copy of its triangles on the card for
as long as the tool is open. It is drawn instead of the flat pass, never over it, and a
hollowed model is drawn flat: the map is indexed by the model's faces and a shell carries
the cavity's as well.

Memory follows the images: a model with five 4-megapixel textures decodes all five into
heightmaps before the first lattice point is displaced. They are dropped with the map once
the relief is pressed.

The window refuses a model whole if it cannot decode one image of several, rather than
pressing the parts it could read. Skipping one would renumber the rest, and the map is
indexed by that numbering.

## Alternatives considered

### One `UvMap` per image, each with holes

No new type, and `press` would take pairs. It lost because every map is as long as the face
list, so five images over a hundred thousand faces cost five times the map for one image's
worth of information.

### Merge the images into one atlas at load

What a renderer does, and it would have kept the old signature. It lost because packing an
atlas is work of its own, and because the coordinates would all have to be rewritten into
it — a second thing that can be wrong, for a consumer that reads one texel at a time and
does not care how many images there are.

### The option that won, and what it costs

An index into a list held somewhere else is a weak link, the same one ADR 0115 accepted for
the map itself: nothing in the type system keeps `UvMap` and `Loaded::textures` together,
and the check that they match happens inside `press`.

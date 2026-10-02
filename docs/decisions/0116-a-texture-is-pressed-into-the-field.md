# 0116. A texture is pressed into the field, not onto the vertices

- **Status:** Accepted
- **Date:** 2026-09-28

## Context

A UV texture has to become something a resin printer can make. It cannot become exposure:
grey controls cure depth, so it could only texture a face pointing up or down, and ADR 0113
takes most of the range away again. So it has to become geometry.

Displacing vertices needs a subdivision this workspace does not have and will not gain for
one feature: the relief of a 128-pixel image on a twelve-triangle box is invisible until
the box has a million triangles. `core-volume` already samples the surface everywhere, on a
lattice fine enough to resolve a wall, and already meshes an isosurface back out.

ADR 0115 left one question open: `cut`, `split` and `hollow` all rewrite the face list a
`UvMap` is indexed by, and none of them carries it through.

## Decision

`core_volume::press` builds the model's field, subtracts the texture's height from every
lattice point — `closest_point` names the face, `UvMap::at` gives the coordinate,
`Heightmap::sample` gives the height — and extracts the surface again. The amplitude is a
signed millimetre distance: positive raises the relief, negative sinks it in. The band is
built a whole amplitude wider than it otherwise would be, so the displaced surface cannot
leave the tiles the field stored, and the result's band is that much narrower.

The image is decoded outside the field: `Texture::decode` in `core-mesh-io` reads PNG and
JPEG through `image` into a `core_geometry::Heightmap` of one height per pixel, taken from
brightness. `Heightmap` lives in `core-geometry` because two crates that may not see each
other need it.

A map lives only as long as the faces it is indexed by. `weld` reports the faces it drops
and the app compares the face list `orient_outward` returned, so `UvMap::without` and
`UvMap::flipping` carry the map through the one repair that touches it. Nothing else does:
the press runs before `cut`, `split` and `hollow`, and it replaces the mesh, so the map goes
with the surface it described. That is the answer to ADR 0115's open question — dropping.

## Consequences

A relief costs a field of the whole model, plus one nearest-point query per lattice point,
and the model comes back as the extraction's own triangles: a twelve-triangle box pressed at
0.6 mm comes out at fifteen thousand. Everything placed on the old surface — supports, the
cavity, a painted patch — is dropped with it, which the panel says before the button is
pressed.

`d - h` is a displaced field, not a distance field. It holds while the amplitude is small
against the curvature it is pressed into; a relief as deep as a feature is wide will fold.
The lattice bounds the detail: the field's own voxel is the finest thing the texture can
say, whatever the image's resolution. A UV map that jumps between two faces — a planar
projection over a cube's edge — jumps the displacement with it and leaves a seam the
extraction cannot close.

The map is not written into an `.encrust` project: a project carries meshes, not the files
they came from. Pressing a texture is a modelling step, done before the project is saved.

## Alternatives considered

### Subdivide and displace the vertices

What a renderer and every modelling tool does, and it keeps the model's own topology. It
lost because an adaptive subdivision is a step of its own — as much work again as this one —
and because the slicer has no use for the topology it would preserve.

### Decode the image inside `core-volume`

Would have saved a type in `core-geometry`. It lost because the field crate has no business
holding an image decoder, and because the decoder belongs where the file is read.

### The option that won, and what it costs

Going through a field rebuilds the whole model at the lattice, so a part with fine detail
somewhere else loses it to the extraction unless the precision is raised over the whole
model. It also means the relief cannot be undone by anything but the undo stack: what comes
back is triangles, with no memory of the surface it was pressed into.

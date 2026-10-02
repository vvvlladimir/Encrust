# 0115. Carry UVs beside the mesh, not inside it

- **Status:** Accepted; its partial-map clause superseded by 0121, its one-texture clause by 0123
- **Date:** 2026-09-27

## Context

Pressing a texture into a model needs two things a `Mesh` does not carry: a texture
coordinate for every point of the surface, and the image those coordinates address. OBJ has
both — `vt` lines and the material file's `map_Kd` — and 3MF has both in its materials
extension.

A UV cannot live on a vertex. `weld` merges vertices within a tolerance, and two faces
meeting at a welded vertex routinely name different corners of a texture: a seam is exactly
that. Welding is the first thing both binaries do on import, so a per-vertex UV would be
destroyed before anything read it.

ADR 0111 decided the OBJ loader never opens the material file, on the grounds that nothing
downstream read it. That is no longer true.

## Decision

`core_geometry::UvMap` holds three coordinates per face, indexed the same way as
`Mesh::faces`, and `UvMap::at` interpolates them by barycentric weight so a point found by
`closest_point` — which returns the face it landed on — gets its UV. It lives in
`core-geometry` because that is where `Mesh` lives and a UV is mesh data, and it derives
`serde` like every other type a project file may come to carry.

`MeshLoader::load` returns `Loaded { mesh, uvs, texture }` rather than a `Mesh`. `Texture`
is the image exactly as the file stored it, undecoded: a loader has no business picking an
image decoder, and the consumer knows which formats it can read.

The OBJ loader reads one `mtllib` from beside the file, for `map_Kd` and nothing else. This
supersedes that clause of ADR 0111; an absent or unreadable material file is still not an
error, and an unresolved `usemtl` still only splits the object.

A file where one object carries `vt` and another does not yields no map at all. A map with
holes in it is worse than none, because a consumer cannot tell an unmapped face from one
mapped to the texture's origin.

## Consequences

Three call sites take `.mesh` and are otherwise unchanged, so nothing that slices behaves
differently. The CLI says what a file carried beyond its triangles, which is the only place
the new data is visible until a tool consumes it.

`UvMap` is a second array that must stay the same length as `Mesh::faces`, and nothing in
the type system holds them together: `cut`, `split` and `hollow` all rewrite the face list
and none of them carry a `UvMap` through. A consumer that maps a cut model will find the
map describing the faces the model used to have. They drop it: the press that consumes a
map runs before any of them, and replaces the mesh the map described. See ADR 0116.

`encrust-app` already has a type called `Imported`, which is why this one is `Loaded`.

## Alternatives considered

### A UV on the vertex, and a weld that splits on disagreement

How a renderer does it. It lost because it makes a texture seam multiply vertices in the
mesh that gets sliced, for a slicer that does not otherwise care, and because it puts
texture knowledge inside `weld`.

### Decode the image in the loader

Would let `Texture` be pixels and a size rather than bytes, which is friendlier. It lost
because `png` is the only decoder in the workspace and a real textured model is as likely to
carry a JPEG; refusing one in the loader would mean refusing the whole file.

### The option that won, and what it costs

A sidecar keyed by position in a `Vec` is the weakest link available: it is correct only
while nobody touches the face list, and four existing operations do. Nothing warns. The
alternative — a type owning both, so a face and its UV cannot drift — would have been better
and would have rewritten every signature in `core-geometry` that takes a `&Mesh`.

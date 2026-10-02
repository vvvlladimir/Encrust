# 0111. Parse OBJ with `tobj`, not by hand

- **Status:** Accepted, the material file clause superseded by 0115
- **Date:** 2026-09-27

## Context

`ObjLoader` returned `MeshIoError::Unimplemented` from step 1 to step 16. Slicing needs
only `v` and `f` out of an OBJ file, which reads like a morning's work, but the format's
face syntax is where the cost hides: a face may carry `v`, `v/vt`, `v//vn` or `v/vt/vn`
per corner, indices may be relative and count back from the last vertex declared, a face
may be a polygon rather than a triangle, and `o`/`g`/`usemtl` each cut the stream into
another object. A parser that gets one of those wrong does not fail — it loads a mesh with
the wrong triangles, and nothing downstream can tell.

`core-mesh-io` already takes `stl_io` for the same reason, so a parser dependency is not a
new kind of thing in this crate. The workspace rule is no dependency without cause; format
parsing is the cause.

## Decision

`ObjLoader` uses `tobj` 4.0 with `default-features = false`, which drops its `ahash`
default so the crate's hashing stays `core-geometry`'s business (ADR 0064).

The loader opens the file itself and calls `load_obj_buf`, not `load_obj`: `load_obj`
collapses an open failure into `LoadError::OpenFileFailed` and loses the `io::Error`, while
`MeshIoError::Io` carries the source and the path. The material loader handed to
`load_obj_buf` returns an empty set without touching the disk, so `mtllib` never opens a
file and an absent `.mtl` cannot fail an import; an unresolved `usemtl` only splits the
object.

Options are `triangulate: true`, `ignore_points: true`, `ignore_lines: true` and
`single_index: false`. Every object in the file is concatenated into one `Mesh` with its
indices shifted, because a plate holds a model, not a scene graph.

## Consequences

OBJ arrives with vertices already shared, unlike STL, so a weld is not what makes the
topology readable — the `MeshLoader` contract already allowed this and now something
exercises it. Triangulation happens in the parser, so `Mesh` never sees a polygon.

`tobj` triangulates only polygons that convert trivially to a triangle fan, so a concave
quad or a non-planar n-gon loads with the wrong triangles. The signal to reopen this is a
real model whose OBJ faces are n-gons and whose loaded mesh fails `diagnose`; the answer
then is `earcutr`, which the workspace already has for the cut face (ADR of step 11d).

UVs are parsed by `tobj` and thrown away here. Step 16b is what carries them, and taking
them out of the same `LoadResult` is a change to one function.

## Alternatives considered

### A hand-written parser

Around 150 lines, no dependency, and total control over the error variants. It lost on the
face grammar above: relative indices and the `v/vt/vn` forms are exactly the places a
hand-rolled reader is wrong silently, and there is no closed-form test that would catch a
mesh that merely loads the wrong triangles.

### `obj-rs`

Types the geometry for a GPU vertex layout, which is not what a slicer wants, and does not
triangulate polygons.

### The option that won, and what it costs

`tobj` is written for renderers: it stores positions as a flat `Vec<Float>` that we walk in
threes and copy into `Vec3`, so an OBJ import allocates its vertices twice. Its
`LoadError` is a bare enum with no position in the file, so `MeshIoError::Malformed` can
say `PositionParseError` but not which line — worse than a hand-written parser would give,
and the reason a user with a broken export gets less help from us than from Blender.

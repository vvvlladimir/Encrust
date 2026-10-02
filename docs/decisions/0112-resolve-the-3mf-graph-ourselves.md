# 0112. Parse 3MF with `threemf2`, and resolve its graph ourselves

- **Status:** Accepted
- **Date:** 2026-09-27

## Context

`ThreeMfLoader` returned `MeshIoError::Unimplemented` from step 1 to step 16. Unlike STL
and OBJ, a 3MF file does not hold geometry where it is used: it is a zip under the Open
Packaging Conventions whose `3D/3dmodel.model` carries a resource graph. A `<build>` lists
items; an item names an object; an object is either a mesh or a set of `<component>`s that
name further objects, to any depth; and either may sit in another model part reached by a
`path` attribute. Placement is a `transform` of twelve numbers applied to a *row* vector,
so its rows are a column-major matrix's columns, and the model's `unit` may be anything
from a micron to a metre.

Two things follow. A parser is worth taking from a crate, because OPC, the namespaces and
the extensions are a specification, not a judgement call. Resolving the graph is not: what
a slicer wants out of it is one mesh in millimetres, and no crate can guess that.

## Decision

`threemf2` 0.4 with `default-features = false` and only `package-memory-optimized-read`,
so the write path, the deprecated speed-optimised reader and `serde` stay out of the tree.
It covers the core specification and the materials extension, which is where 3MF keeps its
UVs, so step 16e does not need a second parser.

The graph is walked here. Every build item is resolved, components are followed to any
depth up to 64, placements compose outermost-last, and the whole build is concatenated into
one `Mesh` scaled from `unit` into millimetres. Three cases are made explicit rather than
quietly wrong:

- A placement whose determinant is negative turns every face inside out, so its winding is
  reversed as the triangles are pushed.
- A triangle naming a vertex the mesh does not have is `Malformed`, not a face `Mesh`
  silently drops.
- A boolean-shape or displacement-mesh object is `Unimplemented`, not an empty mesh.

The 64-deep cap exists because the specification forbids a cycle and nothing in a file
enforces that; without it a crafted file recurses until the stack ends.

## Consequences

3MF now goes through the same `MeshLoader` as STL and OBJ, and `slice` picks it up in a
batch directory. A file of two build items loads as one object of two shells: the plate can
move it, hollow it and support it, but cannot pull the two apart, because the loader
returns a `Mesh` and the scene graph is gone by then. That is the cost of the trait's
return type, not of this parser, and the signal to reopen it is a user wanting a 3MF
assembly to arrive as several plate objects.

Reading is eager: `ThreemfPackage` holds every model part, thumbnail and unknown part in
memory before the walk starts, so a large assembly is paid for twice — once as the parsed
tree, once as the `Mesh`. The crate has a lazy reader if that shows up on a real file.

## Alternatives considered

### `quick-xml` and the `zip` the workspace already has

Full control, and no new dependency of consequence. It lost on surface: OPC relationships,
content types, three namespaces, the `path` indirection and the materials extension are a
lot of specification to re-read, and none of it is slicing.

### `lib3mf`

Also pure Rust and it walks the graph. Its default features pull `clipper2` — a polygon
boolean the workspace has refused twice (ADR 0088, ADR 0089) — plus `parry3d` and
`nalgebra` on versions of its own. Disabling them is possible; inheriting that dependency
opinion for a mesh importer is not worth it.

### `threemf` 0.8, which `threemf2` forked from

Does not follow build items or nested components, so the transforms would have had to be
applied here anyway on top of a thinner model.

### The option that won, and what it costs

`threemf2` is young, at 0.4, and its surface shows it: the reader takes a
`process_sub_models` flag, `Transform::to_column_major_matrix` returns a matrix in the
opposite convention to glam's so it cannot be used at all, and `ObjectKind` is
`#[non_exhaustive]`, so the walk needs an arm for a kind that does not exist yet. A
breaking release is likely, and will land on one file.

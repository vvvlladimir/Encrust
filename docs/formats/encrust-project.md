# The Encrust `.encrust` project

Our own format: a plate as it was left, down to the geometry, so that it opens as the
plate that was saved rather than as one built again from its settings (ADR 0191). Written
and read in `crates/core-engine/src/project.rs`, which has no front end in it, so a
window, a terminal and a browser all open the same file (ADR 0174). What the window
converts to and from it is `crates/encrust-app/src/project/state.rs`.

## Shape of a file

A zip. Deflate, no encryption, no comment.

```
project.json            the manifest, UTF-8
models/0/source.mesh    the first object as it was imported and repaired
models/0/shell.mesh     the shell hollowing built from it, if it was hollowed
models/1/source.mesh    the second object
...
```

`models/<n>/` matches `objects[n]` of the manifest, by position. A `shell.mesh` is written
when and only when that object's `hollow.built` is there, and a file whose manifest names
a mesh the archive does not have is refused.

## `project.json`

`version` is read on its own before anything else, and a file claiming any version but
this build's is refused whole. This build writes version 3. There is no reading of older
ones: version 2 and below hold no built geometry, which is the whole of this format.

| Key | Type |
|---|---|
| `printer`, `resin` | `{ id, profile }`, the catalogue id and the profile itself |
| `slicing` | layer height, `AdaptiveSettings`, `ExposureRange` bands, anti-aliasing, `OutputFormat` |
| `supports` | the group table, the active group, `ProjectSettings` for a fill, the brush |
| `hollow` | what the Hollow tool is set to, not what it built |
| `drain`, `cut`, `array` | what the next hole, the cut plane and the copy grid are set to |
| `plates`, `active_plate` | one name per build plate, and the one the project was left on |
| `objects[]` | name, plate, `Transform`, visibility, the import summary, and what each tool placed |
| `objects[].supports.grown` | every tree the automatic run grew, in the model's own space |
| `objects[].supports.frozen` | every tree a hand took hold of, in the same space |
| `objects[].hollow.built` | the shell standing on the model, or `null` for a solid one |

`built` is what the run measured, not what it was asked for: `wall` is the thickness, mode,
precision and infill asked for, and beside it stand `cavity_faces` — which faces of
`shell.mesh` bound the resin — `cavity_mm3`, the lattice `voxel_mm` the cavity came out on,
whether that lattice was `coarsened` to fit a memory budget, and the `scale` the wall was
measured under. A hole is deepened through that wall on opening, so the same wall has to
come back.

The profile travels with the file, not only its id: a plate has to open the same on a
machine whose catalogue never had that printer. The id is kept beside it so the picker
still shows the profile as the catalogue's when it is.

## A mesh blob

```
"ENCM"          4 bytes
vertex count    u32 le
face count      u32 le
vertices        f32 le x3, per vertex
faces           u32 le x3, per face
```

Not STL, and not re-welded on load. A painted patch is a set of **face indices**
(`core_supports::Region`, ADR 0092), so a format that renumbered or dropped a face would
move every patch on the model. The index buffer is written as it stands.

## What is not in the file

Only what is a cheap pure function of what is: the bounding hierarchies, the meshes of the
support columns, the bodies the drains and channels cut, the painted patches as geometry,
the slice stack, the layer preview, the undo history. `core_engine::project::hollow_of`
and `supports_of` work them out, and are the only two places that do, so every front end
opens a file the same way.

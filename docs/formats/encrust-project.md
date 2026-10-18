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
preview.png             the plate as a picture, 480 x 320, stored rather than deflated
models/0/source.mesh    the first object as it was imported and repaired
models/0/shell.mesh     the shell hollowing built from it, if it was hollowed
models/1/source.mesh    the second object
...
```

`models/<n>/` matches `objects[n]` of the manifest, by position. A `shell.mesh` is written
when and only when that object's `hollow.built` is there, and a file whose manifest names
a mesh the archive does not have is refused. `preview.png` is the start page's picture of
the plate, there for whatever lists the file before opening it (ADR 0261); a file without
one reads, and a plate with nothing on it has none.

## `project.json`

`version` is read on its own before anything else. This build writes version 5. Version 4
is read too: it carried its printer and resin whole, and only their ids are kept. Any other
is refused whole: version 3 and below hold no built geometry or diagnostics.

| Key | Type |
|---|---|
| `printer`, `resin` | the catalogue id, or `null`; never the profile itself (ADR 0261) |
| `slicing` | layer height, `AdaptiveSettings`, `ExposureRange` bands, anti-aliasing, `OutputFormat`; taken up only with the printer |
| `supports` | the group table, the active group, `ProjectSettings` for a fill, the brush |
| `hollow` | what the Hollow tool is set to, not what it built |
| `drain`, `cut`, `array` | what the next hole, the cut plane and the copy grid are set to |
| `cut.parts` | what Split into its parts goes by: the smallest part (ADR 0306); absent in older files and read as the defaults, and the narrow-join fields of files written before it are ignored |
| `cut.joints` | how a cut's halves are joined and what each kind is set to (ADR 0305); absent in older files and read as a plain cut |
| `plates`, `active_plate` | one name per build plate, and the one the project was left on |
| `objects[]` | name, plate, `Transform`, visibility, the import summary, and what each tool placed |
| `objects[].supports.grown` | every tree the automatic run grew, in the model's own space |
| `objects[].supports.grown[].landing.pad_offset` | how far a tree's pad stands off its trunk on the plate, on `frozen` trees too (ADR 0239); absent in older files and read as none |
| `objects[].supports.grown[].landing.on_support` | the tree ends on the trunk of a thicker tree of another group, on `frozen` trees too (ADR 0248); absent in older files and read as false |
| `objects[].supports.raft_lift_mm` | how much of the height in `transform` is the raft's lift (ADR 0238); absent in older files and read as 0 |
| `supports.raft` | the raft every model without its own stands on (ADR 0253); absent in older files, where it is read from the first group's profile |
| `objects[].own_raft` | the raft this model stands on instead (ADR 0253); absent when it keeps none, and in older files |
| `objects[].own_supports` | the profile this model is built to in place of a group's, by group number (ADR 0246); absent when it keeps none, and in older files |
| `objects[].supports.frozen` | every tree a hand took hold of, in the same space |
| `objects[].hollow.built` | the shell standing on the model, or `null` for a solid one |

`built` is what the run measured, not what it was asked for: `wall` is the thickness, mode,
precision and infill asked for, and beside it stand `cavity_faces` — which faces of
`shell.mesh` bound the resin — `cavity_mm3`, the lattice `voxel_mm` the cavity came out on,
whether that lattice was `coarsened` to fit a memory budget, and the `scale` the wall was
measured under. A hole is deepened through that wall on opening, so the same wall has to
come back.

The Relief tool has no entry here: what it is set to is remembered between runs instead,
in `preferences.json` (ADR 0192).

The printer and resin travel by id alone, so a file never changes the profiles of whoever
opens it. Where the catalogue has that id the window takes it, and the plate's slicing with
the printer; where it does not, the window keeps its own printer, resin and slicing and
says so. The command line asks for `--printer` or `--resin` instead, and a browser's
`web-engine` is handed both as TOML (ADR 0261).

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

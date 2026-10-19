# 0305. Outline a label with `ttf-parser` in `core-text`, offer the machine's fonts, and let it follow the surface

- **Status:** Accepted
- **Date:** 2026-10-18

## Context

A user wants a name, a part number or a date on a print, raised out of a face or sunk into
it, in a font they choose, placed where they click — on a flat face, and on a scanned
figure, a bottle or a sphere, where every letter has to keep its depth off the surface under
it. That needs a font read into outlines, the outlines filled into letters with their
counters open, the letters stood up and joined to the model. The join is
`core_volume::combine` (ADR 0304); the filling is the ear clipping `core-geometry` already
caps a cut with (ADR 0089). What is missing is a font reader, and with it a list of fonts to
pick from.

## Decision

A new crate, `core-text`, on `core-geometry` and `ttf-parser` 0.25 alone. It reads a face
from a font file's bytes, lays a line out by advance and `kern`, flattens quadratic and
cubic curves to within 2 µm, nests the rings by how deep each lies in the others whatever
way the font winds them, and stands each filled shape up as a closed prism. `label` lays
them at a `Frame` — a point, the face's normal, and up along the plate's z, or its y on a
level face, turned by an angle.

`LabelSettings::follow_surface` picks how. Flat, the default, casts a ray down the normal
under every corner, taking the face only within the depth of the label's plane, so the
letters never reach more than twice the depth off the plane and a pillar beside them is
never cut. Following, each filled shape's edges are split at their middles until none is
longer than 1/160 of the label's longest side and never under 0.25 mm, split for both
triangles on it so the letter stays closed; under every point a ray goes down the normal,
or up it from inside the model, and the point stands off that height by the depth and the
0.2 mm overlap. Heights beyond half the label's longest side or twice its depth are not
taken; a point with none stands at the mean of the rest. The rays run on `rayon`.

The Relief tool's Text mode offers the bundled IBM Plex Sans at once and, the first time
the font list is opened, every family installed on the machine, read by `fontdb` 0.23 on a
worker; a browser has none to offer. A click on a model stamps the label through
`combine_exact` — union for raised, difference for sunk — and a model it cannot read is
refused rather than remeshed through the field.

## Consequences

Lettering is exact on any closed model, a model of crossing parts included, and refused on
a torn one, which has to be repaired first. The parser stays out of `core-geometry`, which
every crate takes. A following letter keeps its depth along the click's normal, so it is
thinner across a face that turns steeply away, by the cosine, and carries hundreds to
thousands of triangles, which the preview rebuilds each frame and the join has to cut. A
font file is read again when the text changes. Reopen if a letter has to keep its depth
square to a steep face, if the preview stutters on a long label, or if shaping a script
`kern` cannot carry — Arabic, Devanagari — is asked for. `ttf-parser` is marked unmaintained
(RUSTSEC-2026-0192), and `deny.toml` waives it as `fontdb` already ships it; a fix it needs
and will not get is when `core-text` moves to `skrifa`.

## Alternatives considered

### A crate that meshes text whole

`meshtext` and `fontmesh` go from a string to a mesh in one call, but bring a triangulator
of their own, cannot lay letters on a surface, and leave no say over how curves are cut.

### `skrifa`

Also already in the lockfile, and better at CFF2 and variable fonts, but a heavier API that
moves faster; nothing here needs what it adds.

### Offsetting each point along its own normal

Square to the face everywhere, but a scan's normals are noise, and neighbouring offsets
cross in any hollow tighter than the depth.

### Only the bundled font and a file picker

No second dependency, the same in a browser as at the desk. A user's fonts are installed
already, and finding the file of one is the slow part.

### The option that won, and what it costs

Two dependencies, one of them reading every installed face's header the first time the
list opens, which takes a moment on a machine with thousands. No shaping beyond kerning, so
a script that joins its letters comes out unjoined. Splitting edges leaves earcut's slivers
as many smaller ones, more triangles than an even fill would have.

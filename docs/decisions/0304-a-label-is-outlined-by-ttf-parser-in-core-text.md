# 0304. Outline a label with `ttf-parser` in `core-text`, and offer the machine's fonts

- **Status:** Accepted
- **Date:** 2026-10-18

## Context

A user wants a name, a part number or a date on a print, raised out of a face or sunk into
it, in a font they choose, placed where they click. That needs a font read into outlines,
the outlines filled into flat letters with their counters open, the letters stood up and
joined to the model. The join is `core_volume::combine` (ADR 0303); the filling is the ear
clipping `core-geometry` already caps a cut with (ADR 0089). What is missing is a font
reader, and with it a list of fonts to pick from.

## Decision

A new crate, `core-text`, on `core-geometry` and `ttf-parser` 0.25 alone. It reads a face
from a font file's bytes, lays a line out by advance and `kern`, flattens quadratic and
cubic curves to within 2 µm, nests the rings by how deep each lies in the others whatever
way the font winds them, and stands each filled shape up as a closed prism. `label` lays
them at a `Frame` — a point, the face's normal, and up along the plate's z, or its y on a
level face, turned by an angle — and casts a ray down the normal under every corner,
taking the face only within the depth of the label's plane, so the letters stand the depth
out of the face or into it, never reach more than twice the depth off the plane, and overlap
the model by 0.2 mm on the other side.

The Text section of the Relief panel offers the bundled IBM Plex Sans at once and, the
first time the font list is opened, every family installed on the machine, read by
`fontdb` 0.23 on a worker; a browser has none to offer. While its switch is on, a click on
a model stamps the label through `combine_exact` — union for raised, difference for sunk —
and a model it cannot read is refused rather than remeshed through the field.

## Consequences

Lettering is exact on any closed model, a model of crossing parts included, and refused on
a torn one, which has to be repaired first. The parser stays out of `core-geometry`,
which every crate takes. Letters are flat: on a face that curves away by more than the
depth, the corners past it stand off the surface. A font file is read again when the text changes. Reopen if text has to follow
a curved surface, or if shaping a script `kern` cannot carry — Arabic, Devanagari — is
asked for.

## Alternatives considered

### A crate that meshes text whole

`meshtext` and `fontmesh` go from a string to a mesh in one call, but bring a triangulator
of their own, cannot lay letters on a surface, and leave no say over how curves are cut.

### `skrifa`

Also already in the lockfile, and better at CFF2 and variable fonts, but a heavier API
that moves faster; nothing here needs what it adds.

### Only the bundled font and a file picker

No second dependency, the same in a browser as at the desk. A user's fonts are installed
already, and finding the file of one is the slow part.

### The option that won, and what it costs

Two dependencies, one of them reading every installed face's header the first time the
list opens, which takes a moment on a machine with thousands. No shaping beyond kerning,
so a script that joins its letters comes out unjoined.

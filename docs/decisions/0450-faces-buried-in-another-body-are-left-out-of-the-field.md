# 0450. Leave out of the field the faces a closed body of the surface buries

- **Status:** Accepted
- **Date:** 2026-11-16

## Context

A model built of parts carries them as shells that overlap: an arm pushed into a torso, a
plaque laid on a base. The field of a hollowing run measures the distance to the nearest face
of the shells that bound the solid (ADR 0186), so a face of one part lying inside another
cuts a wall of the cavity's thickness all along it. `bob.encrust`, 55 shells and 4.1 million
faces, came out with 19 separate cavities, each one needing its own drain, and with walls
through the figure where the parts meet.

The sign is already the winding number of the whole mesh whenever more than one shell
bounds the solid (ADR 0448), and that number reads the union of the parts: two inside an
overlap, one in either part alone. Only the distance still sees the parts one by one.

Three things a first attempt got wrong, each costing `bob` a sixth of its cavity:
a face over a hole in an open part closes that hole; a part `outer_shells` leaves out of
the field, because most of it lies inside others, cannot be what buries a face, or nothing
bounds the solid there; and a face standing on another's face reads as on that face, not
inside it, unless it is asked a little off itself.

## Decision

Where two or more shells bound the solid and the sign is not forced to the pseudonormal,
`outer_shells` drops each face of a bounding shell that a closed bounding shell holds
(`shells/buried.rs`). A face is held when, a thousandth of its own shell's diagonal outside
it, the winding numbers of the closed shells whose boxes reach that point sum past one half
at its middle and at each corner drawn a tenth toward the middle. Only the faces of a shell
whose box meets another's are asked, and a closed shell wound inward, the void of a model
already hollow, buries nothing and is buried by nothing. The faces stay in the model and print as before.

## Consequences

`bob` hollows as 16 cavities instead of 19, with 0.6 % more resin saved, at 13.1 s of field
instead of 11.0 s; its large parts are mostly ones `outer_shells` already leaves out, so the
rest of its walls stay. Two boxes overlapping by half hollow as one box, to the volume of
their union. A model with crumbs on its surface pays a winding hierarchy of the shell they
touch: `niko.stl` 4.1 s instead of 3.9 s. A model of one shell pays nothing.

The surface the field is built over is open where faces were dropped. That is sound only
because the sign is the whole mesh's winding number, which is why a forced pseudonormal
keeps every face.

Reopen it if a model of parts that are mostly enclosed by others turns out common: those
parts are left out of the field whole, and their faces outside the others are lost with them.

## Alternatives considered

### The whole mesh's winding number just outside each face

One hierarchy for every question, already built. It reads an open part's hole and a part
left out of the field as solid, and dropped the faces that close them: a sixth of `bob`'s
cavity went with the floor of its torso.

### Cut the shells against each other exactly

A boolean union in `core-csg` would give the true surface, seams included. It costs an
exact intersection of millions of faces on every hollowing, and fails on the open meshes
this path exists for.

### Only count the cavities and show the number

Cheap, and it tells the user how many drains to place, but it leaves every wall standing.

### The option that won, and what it costs

A winding hierarchy for each closed shell that meets another, and up to four questions for
each face in an overlap: a fifth more field time on `bob`. A face half inside another part
is kept, so a short wall stays along each seam.

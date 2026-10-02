# 0042. Support shapes are data in the profile, swept by the one mesher

- **Status:** Accepted
- **Date:** 2026-09-19

## Context

Every MSLA slicer lets the user pick what the tip and the foot of a support are shaped
like — a contact shape and a handful of platform shapes — and the numbers beside them are
what a user carries between slicers. Step 7d is about carrying that set of
parameters, so the shapes have to arrive with it.

The project already has two commitments that bear on how.

`docs/decisions/0041` meshes a support as a set of overlapping closed tubes, each a
solid of revolution about its own axis, described as a list of rings. That is a narrow
vocabulary: it can express anything rotationally symmetric, and nothing else.

`AGENTS.md` rule 2 says to extend through traits rather than enum
switches, and rule 4 says data is separate from algorithms. A shape, though, is neither a
file format nor a generator — it is a field of a profile that a user types into a TOML
file and that has to round-trip through `serde`.

There is also a shape the parameter set implies but the ring vocabulary cannot hold. A
*skate* foot is a pad cocked up at one end so a blade gets under it. It has no axis of
revolution.

## Decision

Shapes are **plain data in `printer-profiles`** and are **meshed by the existing ring
sweep in `core-supports`**. No trait, no new mesher.

`ContactShape` is `Cone`, `Sphere` or `Plane`. `PlatformShape` is `Cylinder`, `Cone`,
`Prism` or `Cube`. Both are `#[serde(rename_all = "kebab-case")]` enums with a `Default`,
so an older profile that names no shape keeps the shape it used to have.

Every one of them is expressed in the ring vocabulary:

- a cone tip is an apex `bite_mm` above the contact, as in `docs/decisions/0028`;
- a sphere tip shares that apex as its upper pole and adds the rings between;
- a plane tip has no apex at all, and the sweep closes it with the flat disc it already
  draws when there is none;
- a foot's shape is how many sides its rings have and whether it narrows upwards.
  `PlatformShape::sides` and `PlatformShape::tapers` are the whole of it.

A foot needs a different side count from the support above it, and one tube has one side
count, so **the foot becomes a tube of its own**. The trunk is run the whole way down to
the plate through it, which is what makes the two overlap by `docs/decisions/0041`.

`platform_diameter_mm` is the circle the foot's corners sit on, so a cube's sides measure
that diameter over the square root of two.

Skate feet are **not** in this step. They are in step 7e with rafts and cross bracing.

The thin strut between two parts of the model — what MSLA slicers call a *small pillar* —
is a `[small_pillar]` section rather than a shape. Which supports use it is **derived, not
marked**: a support is thinned when it lands on the model, carries one tip, and the section
is enabled. A trunk that several tips hang off keeps its own width whatever it stands on,
because it carries their weight.

## Consequences

Adding a shape is adding a variant and a branch in `head` or `foot`. There is no new
type to implement, nothing to register, and a profile that names an unknown shape fails
at `serde` with the path of the file, which is where a typo belongs.

The shapes are exactly the ones a surface of revolution can hold, and the ADR says which
one that excludes and where it went instead. If step 7e's skate needs a second primitive,
this decision is not in its way: a skate is a new `Tube`-shaped thing beside the sweep,
not a change to the sweep.

A sphere tip's rings push the top segment down to where the ball ends, because rings never
climb back along the axis. The neck is therefore slightly shorter than `top.length_mm`
measured from the contact. That is right — the ball is occupying that room — but it means
the neck length is not exactly the number in the profile for a sphere tip.

Deriving the small pillar rather than marking it means a user cannot ask for a thin strut
on a support that reaches the plate, and cannot refuse one on a support that does not. The
switch is all or nothing. Reopen this if anyone needs per-support control, which is a
selection model this project does not have yet.

## Alternatives considered

### A `TipShape` trait in `core-supports`, implemented once per shape

What rule 2 asks for at first reading, and it would keep `head` free of matches.
Rejected because a shape has to live in the profile to be serialised, and
`printer-profiles` is a leaf crate that must stay dependency-light — it cannot hold
geometry, and a trait object in `core-supports` keyed by an enum in `printer-profiles` is
the enum switch again with an indirection on top. Rule 2 is about a second *implementation
of a behaviour* arriving; a shape is a field.

### Polygonise the tip and the foot from a distance field

Every shape, including the skate, falls out of one code path, and the joints come out
filleted as a bonus. Rejected as a whole subsystem for a step whose subject is a
parameter list, and for the same reason `docs/decisions/0041` rejected it.

### Enums in the profile, swept by the one mesher, which is what we do

The honest costs are the three above: the shape set is limited to solids of revolution and
a side count, so the skate had to be pushed to another step; the sphere tip quietly
shortens the neck; and the small pillar is derived, so it cannot be asked for or refused
per support. The foot becoming its own solid also adds one cap per support to the face
count and makes the support mesh's volume over-read by the pillar inside the foot, which
the mesher's own volume test now states in its closed form.

# 0358. A closed shell inside another follows its holder's turning

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

`orient_outward` turned every closed shell whose signed volume was negative, each on its
own. It runs on every import and in every repair, so the cavity of a model hollowed before
export — an inner shell wound inward — was turned outward on load: the hollow became a body
inside the model and printed solid. ADR 0352 left this standing as "a decision about
orientation, not about wrapping", and its wrap swallowed what was by then a body.

The engine already reads a mesh with a shell wound inward whole, as a part with a cavity
(`core-volume::boolean::solid_of`), and the fill rule counts it out; only orientation broke it.

## Decision

**A closed shell inside no other is turned when its volume is negative; one inside another
follows its holder, the smallest closed shell around it: turned when the holder is, kept
otherwise.** The file's winding of a nested shell relative to its holder stands: inward is a
cavity, outward a body inside a body.

The holders are looked for only when some closed shell is wound inward, since otherwise none
turns. Candidates are shells boxed inside another's bounds, by a sweep along x; each casts a
ray along the three axes through a `Bvh` of the boxing shells' faces, and an odd count of
crossings on two of three rays puts it inside. See `docs/design/mesh-repair.md`.

## Consequences

- A hollow model keeps its cavity on import, through repair, and through a wrap, which now
  keeps it rather than swallowing it.
- A hollow model written wholly inside out turns with its cavity and stays hollow.
- A body standing wholly inside another stays a body; the wrap still swallows it.
- A model with every closed shell wound outward costs nothing more: 3.12 s before, 3.17 s
  after on `inspect` of the 4.1 M-face colour mix. A hollow ball of 400 k faces inside out
  orients in 26 ms (`benches/mesh_repair.rs`).
- A shell overlapping another without lying inside it can be taken as held when its point
  falls in the overlap and its bounds inside the other's. Reopen if such a model turns up
  wound inward.

## Alternatives considered

### Decide every shell by the parity of the shells around it

Inside an odd number of closed shells is a cavity, so wound inward. Rejected: a part standing
inside another, written outward, would be turned into a hole — a change of what the model
encloses that the file never asked for.

### Leave a nested shell exactly as the file wound it

Rejected: a hollow model written inside out would turn its outer wall and leave the cavity
wound outward, a body inside a body; following the holder turns both.

### The option that won, and what it costs

A ray cast per nested shell and a hierarchy over the shells that box another, paid only when
some closed shell is wound inward; and a shell partly overlapping another may be judged held.

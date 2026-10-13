# 0251. Stand a small pillar on the support beside it, never a support on a small pillar, and build nothing inside the model

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

A thinner group runs its struts into what a thicker group has stood, and which is thicker
is read off the nodes (ADR 0248). A tree that lands on the model with one tip is not built
to its nodes: it is the small pillar, the thinnest strut on the plate whatever its group
(ADR 0042). So a regular support of a thinner group was leant onto a small pillar of a
thicker one, a strut several times wider than what it stood on, while the pillar itself
still ran down to the part and left its mark there.

The strut a stranded tip is given straight down (ADR 0249) took the first face under the
contact. For a contact inside the model — an automatic one is put a head's radius in from
the outline, which on a steep underside is inside the skin — that face is the inside of
the surface below, and the strut was built in the solid with its end poking out.

## Decision

- What a tree can carry goes by the width it is built to: a small pillar is a carrier
  only as wide as `[small_pillar]`.
- Once every group has been grown, each small pillar that has a neck is run into a
  carrier wider than itself where one is in its reach, by the rule of ADR 0248, and is
  then that thin all the way up: its nodes take the small pillar's radius and its landing
  is `on_support`.
- The strut for a stranded tip lands only on a face that looks up at the contact.

## Consequences

A small pillar beside a support ends on the support and marks the part once, at its tip. A
contact inside the model that can neither stand nor lean gets nothing, which is what it
can hold. A small pillar leant onto a support has a regular tip rather than the plain end
of one standing on the model, since it is no longer between two faces of the part.

## Alternatives considered

### Growing small pillars first, as their own thinnest group

Which trees are small pillars is only known once they have landed, so there is no group
to grow first.

### Snapping a contact inside the model back onto its surface

Would let the strut be built, but where the contact goes is the placement's to say, and a
tip moved out to the skin is a tip on the edge the inset was made to avoid.

### The option that won, and what it costs

A second pass over the forest after every group is grown, and a small pillar's width now
decided in two places: the mesher for one on the model, the nodes for one on a support.

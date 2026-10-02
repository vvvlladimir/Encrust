# 0081. A support holds only the part it stands under

- **Status:** Accepted
- **Date:** 2026-09-24

## Context

ADR 0079 made the reach of a standing support a ball, and named what it did not fix:
coverage was measured on the plate, so a support suppressed samples anywhere within its
radius whether or not the material there had anything to do with it. A trunk running up
past an unrelated ledge left that ledge a support short, and the ledge is where the peel
pulls hardest.

The established answer carries the supported points of a layer part onto the parts above
it — each previous part's grid is copied forward, and two parts growing together join
theirs — so a point only ever suppresses a sample on the same chain of parts.

The density knob has the same shape of error. `spacing = carry * 3 / density` divides a
distance, so doubling the density puts down four times as many supports over an area.
Upstream scales the radius as `sqrt(radius² / density_relative)`, which makes the number
a count per unit area, which is what a user setting it means by it.

## Decision

`Field::parts` returns the pieces of a layer with the piece number of each span kept, and
`Parts::parents` maps each piece onto the pieces of the layer below it, corner contact
included. Both walk the run-length rows once, so parentage costs the outline rather than
the area.

Placement keeps one set of standing supports **per piece** of the layer it has reached. A
piece inherits the sets of the pieces it grew out of, merged where several do; a piece with
no parent — an island — starts empty. A sample is only ever asked about the set of the
piece it falls in. A point further below than the coverage radius is dropped as the run
climbs, since the ball says it can hold nothing above, which is what keeps memory to the
supports that still matter rather than to every one placed.

Supports handed to the run are taken on by the piece they sit on when the climb reaches
their height, so a manually placed support no longer suppresses samples below itself.

`density` divides the spacing by its own root, so it counts supports over an area. The
shipped `light` and `heavy` densities are re-read against the new meaning — 0.6 and 2.5 —
so that the three presets keep their order.

## Consequences

A column passing unrelated material no longer holds it: stacked islands, a ledge beside a
trunk and a part standing over another part are all held on their own account. More
supports go down on anything with parts that pass close to each other.

Peak memory now follows the live parts of one layer rather than the whole run, which is
strictly better than the single plate-wide set it replaces. The cost is the copy: a piece
that splits in two hands each half its own copy of the set, so a model that keeps splitting
copies more than one that does not.

Placement now builds a labelling per layer as well as a field, one pass over the spans, in
the same parallel block as the field it labels. `cargo bench -p core-supports --bench place`
pays for it: 5.10 to 6.39 ms on the 1200-layer tables, 9.32 to 10.06 ms on the 600-layer
ball, 11.80 to 12.32 ms on the million-face one. A piece that is the only child of its only
parent takes the set over rather than copying it, and a set with nothing below the floor is
not walked at all, which is what keeps a wall going straight up free.

The part a sample belongs to is the part on the layer being sampled. Two pieces that are
one body lower down, separated only above, are two parts here — which is what makes a
bore's two sides hold themselves.

## Alternatives considered

### Test membership geometrically instead of tracking parts

Ask, for each standing support and each sample, whether the two are connected on this
layer. That is a flood fill per query against a field that is already run-length, and it
answers nothing the labelling does not, at a cost that follows the area.

### Bind a support to the part it was placed under and keep it for the whole run

One number per support and no per-layer parentage. It is wrong exactly where parts merge:
two bodies that grow together are one body from there up, and a support under one of them
does hold the other.

### The option that won, and what it costs

A part is a connected piece of one layer, which is a slice of the truth: two pieces of the
same body that have just split still share everything below, and the moment they split they
stop sharing supports. That is the conservative direction — the extra support goes down —
but on a model that splits and rejoins repeatedly, such as a lattice infill seen in section,
it puts down more than a body-wide notion of a part would.

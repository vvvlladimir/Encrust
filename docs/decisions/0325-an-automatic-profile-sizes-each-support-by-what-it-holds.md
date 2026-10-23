# 0325. Size each support by what it holds, through an automatic profile over groups

- **Status:** Accepted
- **Date:** 2026-10-22

## Context

A placement run builds every support it puts down to the group in hand (ADR 0094), so one
shape holds a needle tip and the underside of a bust alike. A light shape fails under the
bust; a heavy one scars the tip. What a support needs depends on what it holds: the area of
the island it is the first ground of, or how much of the model stands above the overhang it
is under. The first is known where the island begins; the second only once the stack above
it has been read, which a run that streams layers upwards (ADR 0033) has not done yet.

## Decision

**An automatic profile** (`AutoProfile`, `assets/profiles/auto/`) names three support
profiles of the catalogue by id, lightest first, the placement numbers a run uses — density,
overhang angle and the two spacings — and two thresholds per measure: islands of 2.4 and
3.2 mm², overhangs under 1000 and 2000 mm³. It ships with three shapes, `auto-light`,
`auto-medium` and `auto-heavy`: one 1.2 mm pillar, contacts of 0.24, 0.30 and 0.40 mm. It
is bundled only; the user tunes the shapes it names.

**Placement says why.** `generate_supports` answers each contact with a `Reason`: an island
with its area, an overhang with the model above it, or a support that steadies a part. Only
an island's size would move where supports go, and its spacing already follows its area —
by the support profile's `[islands]` areas and shares, the two areas taken from the
automatic profile's own when it places, so one number picks both size and spacing; so
placement runs as before and the model above an overhang is settled after the climb. The
pieces of each layer are followed onto those of the layer before as a graph whose node runs
on while a piece neither splits nor joins, so it grows with the model's branchings, not its
layers; a piece grown out of several hands each an even share.

**The window maps reasons onto groups.** A switch across the top of the Supports, Paint
and Shape tools sizes supports automatically or by hand, and each tool's list offers only
the shapes of the side switched to: a support profile an automatic profile names is an
automatic shape, any other is chosen by hand. Sizing automatically, the Supports tool's list
offers the automatic profiles themselves, its fields edit where the shapes change, and its run puts each contact into the group built
to the shape its size names: one of that profile's name, else a group nothing stands in,
retuned, else a new one. A size nothing takes gets no group.

**Tips of one width merge across groups.** Groups whose pillars are within 0.05 mm of one
width and which branch grow as one family: their fronts meet within both groups' branch
angles and merge distances, clear by the larger clearance, and a joint is built to the
group with the heavier head — the larger contact, then the wider pillar, then the lower
number. A tree is stamped with that group, which builds its trunk, foot and braces; a tip
of another group names its own in `TreeNode::head`, which the mesher builds its head to.
The field is left out of a file where it is empty and read as empty from an older one, so
the manifest stays version 4. Dropping or emptying a group cuts its tips off the trees of
other groups and takes the trees built to it whole. Families of different widths are grown
thickest first and lean onto each other as ADR 0248 has it.

## Consequences

A mixed plate takes a heavy support only where a large island or much of the model asks for
one, and the three shapes share trunks, feet and braces as one group's tips would. A low
overhang under a tall single body carries all of it, so most of a statue's lower supports
are heavy; that is the measure as chosen, to be checked against prints. The graph keeps a
volume and its parents per node to the end of the run. A strut of a merged tree is drawn and hidden with the heaviest head hanging from it, so a light tip keeps its colour down to the joint and the trunk takes the heavy one's. Two groups a user made of one width now share
trunks too, where before they stood apart. Steps, stabilising supports and saving an edited
automatic profile are not here: a support that steadies a part takes the lightest shape
until it has a shape of its own.

## Alternatives considered

### A diameter on every support

Exact per support, but a new field in `.encrust` and trees that cannot share a trunk
(ADR 0094).

### Sizing while placing

One pass, but an overhang's load is unknown until the top of the stack.

### A lighter head leaning onto a trunk as wide

The carriers of ADR 0248 let in under a heavier head: no new field, but every light tip
beside a heavy one stood a strut of its own into it, a forest of struts where one trunk
would do.

### Merging every group whatever its width

The fewest trunks, but a joint leans both fronts towards each other, so a thick trunk would
be bent towards a thin tip; a carrier keeps the thick one standing straight.

### The option that won, and what it costs

Three groups where there was one, a run that holds its graph to the end, and a tree whose
tips may each be built to another group, which dropping a group and moving a tree have to
mind.

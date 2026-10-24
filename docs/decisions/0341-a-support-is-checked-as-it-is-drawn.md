# 0341. Check a support as it is drawn, fill a beam with balls, and leave out what still meets the part

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

Users still found supports cured into the part past their tips: a head lying in a crevice,
a trunk beside a rib, a joint's dome outside the strut that was tested. Three causes.
A beam is sampled by nine rays (ADR 0077), and a rib or a needle thinner than the gap
between two of them, inside the body, is never met. Five places — the column, the merge,
the lean, the hand's carry and the pad — each test their own idea of a body, while the
mesher draws another: a trunk widening to its base, a dome round each joint, a head built
to the group that carries it. And a head that cannot turn clear is kept where it is
(ADR 0328), so it is built into the model by design.

## Decision

**A beam is filled with balls.** `clear` keeps the rays of ADR 0077, which find a wall a
beam crosses and a start inside the solid, and adds a row of balls down the axis: each as
wide as the beam at that point and over its own length, no wider than its distance to the
nearer end, half a radius apart, so the row covers the beam to within 3% of its radius. A
ball is tested by `Placed::reaches_ball`: the `Bvh` gathers the faces near it in the
model's own space, with the radius stretched by the Frobenius norm of the inverse
placement, and each is measured exactly on the plate. No new query in `core-geometry`.

**A grown tree is tested as it is drawn.** `mesher::bodies` builds the tubes the mesher
sweeps, struts and trunk, with the outward normal of the face each end is built to meet,
and `mesher::sound` tests every ring-to-ring frustum of them, 0.05 mm narrower (a graze one
layer deep is two facets touching), starting from where the tube stands that graze off its
tip's face and ending where it stands as far off the face its foot stands on. A tube that
never leaves its face lies along it and fails. The pad is not in it: ADR 0328 and 0340
already place it against the model, and a cut pad is not a ring tube.

**What fails is left out, not cured in.** After each family has grown and before thinner
groups can run into it, a tree that is not sound is mended: the tips whose own branches
meet the model are grown alone and the rest are grown together again, up to three times;
a tree whose trunk or an inner strut meets it is grown again tip by tip. A tip that
still fails on its own is left unstood, and the inspector counts it with the others that
have no room. Taking a whole tree apart for one bad head turned a figurine's 12 trees into
43 pillars; mending keeps it at 15. A small pillar is moved onto a support only when the moved
tree is sound. A dragged support is held to the same test.

This supersedes the sampling of ADR 0077 by rays alone and the clause of ADR 0328 that
lets a head with no clear direction stand anyway.

## Consequences

No support of a grown forest meets the part beyond its tip, to the graze, whatever the
placement checks predicted; a head in a crevice too narrow to turn in is now reported
rather than printed into the model. More points can show as having no room on a cramped
model; the longer search of ADR 0342 wins some back.

`cargo bench -p core-supports --bench branch`: growing the lifted 40 mm ball went from
2.72 ms to 6.48 ms and the one on the plate from 70.5 ms to 81.8 ms, the same 10 trunks for
161 tips, and `--bench place` moved within 2%. Reopen if growing a dense plate becomes the
slow step of a run.

## Alternatives considered

### A swept-cone distance query in `core-geometry`

Exact, as ADR 0077 noted, but a new traversal against a hierarchy built for rays; balls
reuse `faces_within` and reach the same answer to within the graze.

### Test only the drawn bodies, and drop the placement checks

One test instead of two, but placement needs answers before a tree exists — where a front
lands, which merge to take — and would then build trees only to throw them away.

### The option that won, and what it costs

Placement still predicts with beams and the drawn bodies are checked after, so the two
are separate code that can disagree; when they do, a support is grown twice or left out
rather than printed wrong. The balls cost up to 2.4 times the growing time.

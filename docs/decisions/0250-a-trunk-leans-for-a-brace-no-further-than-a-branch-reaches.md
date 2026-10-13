# 0250. Lean a trunk for a brace no further than a branch reaches, towards any group, and tie a brace to the trunk's own axis

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

ADR 0240 leans a tall trunk standing out of a brace's reach towards its nearest neighbour
until one reaches. There was no limit on how far: inside a hollow part a trunk by one wall
was carried on a knee right across the cavity to stand beside trunks at the other, a strut
longer and less steady than the trunk the brace was meant to hold. The neighbour was
looked for in the trunk's own group alone, though a brace ties any two trunks (ADR 0094).

A brace ran between the points under two roots. A trunk whose foot or root a hand had
carried (ADR 0095) leans, so its braces ended in the air beside it.

## Decision

- The lean for a brace is taken only where it covers no more ground than half the
  profile's merge distance: what one strut of a merge covers.
- The neighbour may be a trunk of a thicker group, which has been stood already. A group
  is gathered before the next one is grown, so nothing thinner has been leant onto a
  trunk (ADR 0248) by the time it moves.
- Whether a trunk can be tied, and how far a brace spans, are asked of the ground
  profile, which is the one the braces are built from.
- A brace ends on each trunk's own axis at that height (`trunk_axis_at`).

## Consequences

A trunk far from every other stands alone, unbraced, under its own tips. Trunks of two
groups stand close enough to be tied where one could lean to the other. A support edited
by hand keeps its braces. Reopen this if lone tall trunks are seen to sway: the answer is
then a wider base, not a longer lean.

## Alternatives considered

### Leaning as far as it takes, at 45 degrees

The lean of ADR 0249 keeps the strut printable and no shorter; a cavity 40 mm across still
gets a 55 mm diagonal.

### Dropping the lean altogether

Loses the common case it was made for: two trunks a little too far apart to tie.

### The option that won, and what it costs

A thinner group can lean towards a thicker one but not the other way, since the thicker is
stood first.

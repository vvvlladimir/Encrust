# 0239. Make a pad as wide as its profile says, and move it rather than narrow it

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

ADR 0237 narrowed each pad to leave half a millimetre of air to its nearest neighbour and
to stay out of the model. In the window both read as a fault: supports of one profile
stand on pads of different sizes, a pad beside the part shrinks to a button that holds
nothing on the plate, and the Diameter field no longer says how wide a pad is.

## Decision

**A pad is `platform_diameter_mm` wide, always.** Neighbouring pads overlap into one, and
the fill rule unions them (ADR 0041).

**A pad the model is in the way of is moved aside whole.** `Landing` gains `pad_offset`,
how far the pad's middle stands off the trunk on the plate. The landing tries the pad
under the trunk, then at three reaches in eight directions, no further than keeps the
flare inside the pad's sole, and takes the first that clears the model. Only a round pad
that does not taper has that room. Where nothing clears, there is no landing, and the
support leans for one as any blocked front does (ADR 0078) or is dropped and counted.

## Consequences

- The panel's number is the pad's width again, and no pad is cured into the part
  (ADR 0077).
- A dense run stands on one slab: on `peel-slab` laid flat with Medium that layer pulled
  261 N against a limit of 30 N, and under a ring of supports it is a closed cup. The
  analysis reports both; nothing prevents them.
- A support low over the plate beside a part may have no room at all and is dropped.
- `pad_offset` is in the project file; one written without it reads as pads under their
  trunks.
- Reopen when the merged pads tear a print off: the answer is a slotted raft or a profile
  field for the gap, not a silent narrowing.

## Alternatives considered

### Keep the gap between neighbours, or narrow against the model

What ADR 0237 decided, and what this replaces: both trade a number the user set for a
risk the user was not shown.

### Let a pad run into the model

Strictest reading of the setting. It cures the pad onto a part that stands on the plate,
which ADR 0077 forbids for every other piece of a support.

### Moving the pad, and what it costs

A trunk may stand off the middle of its pad, which loads the pad unevenly on the peel,
and the suction and the peel of a merged pad layer are back for the user to answer with
a smaller diameter.

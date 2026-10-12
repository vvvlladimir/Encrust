# 0239. Make a pad as wide as its profile says

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

ADR 0237 narrowed each pad to leave half a millimetre of air to its nearest neighbour, to
keep a dense run from standing on one slab. In the window that reads as a fault: two
supports of one profile stand on pads of different sizes, and the Diameter field no longer
says how wide a pad is. The number a user types is the number they expect to see.

## Decision

**A pad is `platform_diameter_mm` wide, whatever stands beside it.** Neighbouring pads
overlap, and the fill rule unions them (ADR 0041).

**It is narrowed only where the model itself is in its way** — a part standing on the
plate inside the pad's reach — tried at the whole, three quarters and half of its width
and never below the flare it carries. A landing still asks only whether that narrowest
foot clears the model, so such a support stands rather than being dropped, and no pad is
cured into the part (ADR 0077).

## Consequences

- The panel's number is the pad's width again.
- A dense run stands on one slab: on `peel-slab` laid flat with Medium that layer pulled
  261 N against a limit of 30 N, and under a ring of supports it is a closed cup. The
  analysis reports both; nothing prevents them.
- Reopen when that slab tears a print off its supports: the answer is a slotted raft or a
  profile field for the gap, not a silent narrowing.

## Alternatives considered

### Keep the gap between neighbours

What ADR 0237 decided, and what this replaces: it trades a number the user set for a
risk the user was not shown.

### Let a pad run into the model as well

Strictest reading of the setting. It cures the pad onto a part that stands on the plate,
which ADR 0077 forbids for every other piece of a support.

### A pad as wide as it says, and what it costs

The suction and the peel of a merged pad layer are back, and they are the user's to see
in the report and to answer with a smaller diameter.

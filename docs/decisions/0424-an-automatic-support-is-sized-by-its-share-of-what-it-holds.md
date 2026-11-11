# 0424. Size each automatic support by its share of the model it holds

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

ADR 0423 built every automatic tip at the lightest shape and left its size to strengthening
(ADR 0422). The film's pull alone never asked for more, so Auto Medium
and Auto Heavy no longer appeared, even under a 100 ml figure. The pull is the one failure
that read measures. Stiffness under the peel, thousands of cycles on green resin, a torn tip
handing its load to its neighbours, and the wash are not measured, and those are what a
heavier shape under a heavy part is for.

The rule of ADR 0325 sized by the whole model above an overhang and by an island's area. So
the same weight made every tip under it heavy, however many shared it. One figure took 998
tips of 0.40 mm, under its broad base as much as under a cape held at a few points.

## Decision

- **A contact is sized by its share.** The graph of what stands on what (`load.rs`) shares
  the model above each island or overhang contact among the contacts holding it: those on
  its node at or above its layer, itself included, and those on every node grown out of it,
  split as resin is split (`Load::shares`). `Reason::Island` and `Reason::Overhang` carry it
  as `share_mm3`.
- **One threshold set for islands and overhangs.** The automatic profile's `share_mm3`
  replaces `overhang_volume_mm3`. Auto ships 100 and 500 mm³ for its medium and heavy
  shapes, chosen from the bench's spread of shares, where 100 mm³ is its upper quarter and
  500 mm³ its upper tenth. A profile saved before this reads Auto's numbers.
  `island_area_mm2` only spaces islands now. In the window its rows move from Sizes to
  Placing, and Sizes gets one row of share per heavier shape.
- **Steps keep ADR 0326's rule, and strengthening stays on top** (ADR 0422).

## Consequences

Across the bench's thirteen models with Auto, the supports take 44.1 ml against 42.0 with
every tip lightest, and leave 741 mm² of scar against 703. The heavy shapes go where few
contacts hold much: 98 on one figure, 54 on another. The
medium shapes go to figures: 125 on one, 106 on another. The 900 contacts under the first
figure's base hold 66 mm³ each and stay light. Test parts of a millilitre or two keep their
light tips, apart from steps. Eight more of that figure's 1052 contacts are left without a
column, since a heavy head needs more room. Contacts left over by the speck check, and those
of steps and steadying supports, are not counted among those holding a share, so a share is
an upper bound.

The thresholds are proportions of one bench, not a measurement. A print of a heavy figure
both ways, or a heavier shape tearing or swaying where its share called it light, reopens
them.

## Alternatives considered

### Keep ADR 0423

Auto would use its medium and heavy shapes only where the pull read asks for them, and
that read measures one failure out of several.

### Restore ADR 0325 as it was

The whole model above each tip made every tip of a statue's base heavy, which is the scarring
ADR 0423 set out to end.

### The option that won, and what it costs

A share is a proxy for weight, not a force. The thresholds came from where shares fall on
the bench rather than from a failure, and a base of many contacts stays light however heavy
the part is.

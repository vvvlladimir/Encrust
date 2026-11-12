# 0426. Sweep a head, and test a drawn tip, from past the seat its rim finds

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

ADR 0339 sweeps a head from a hair (0.01 mm) past its pin and tests a drawn tip from a graze
(0.05 mm) off the plane of its face. Both read the face as flat. Under a tip of Medium's or
Heavy's width on a curved or coarsely faceted underside — a cape's folds, a helmet's rim — the
face under the rim rises 0.01–0.13 mm towards the head: the tip's own seat, which ADR 0339's
seat test already accepts. The sweep read that seat as the model in the head's way, so the
support was dropped and counted as having no room to stand. On the bench (Heavy, no
strengthening) that was 22 of 22 unheld supports on one model, 20 of 23 on another.

## Decision

`seat_mm` measures, at the 8 rim points the seat test already casts, how far the face rises
towards the head along its normal, none where it falls away. `head_clear` starts its sweep
that much further along the axis, over the cosine of the head's lean; `sound` asks a drawn tip
to stand that much further off its face's plane before it is tested. The seat is bounded by
the seat test itself: half the contact's radius.

## Consequences

Heavy on the bench: unheld supports 22 → 3, 26 → 13, 23 → 6, 20 → 4 on the four figures;
Medium 22 → 3, 12 → 5, 15 → 4, 9 → 2. Automatic placement's light tips seat within the hair
already and are unchanged; scar area within 0.5 per cent. Over a seat the head is a contact's
width plus at most an eighth of the seat's height of widening, so the mark stays the contact's
circle. Each head sweep and each drawn tip costs 8 rays more. Reopen if marks wider than the
contact show where a support stands on a curved underside.

## Alternatives considered

### A larger fixed hair

A constant large enough for a coarse fold lets a head beside a real lip through, which ADR 0339
refuses; the measured seat is zero there.

### The option that won, and what it costs

The rim casts are spent twice, once to seat the tip and once to sweep it; a face that rises
between the rim points is still read as in the way.

# 0443. An edit keeps every head square to its face and every support standing on another seated

- **Status:** Proposed
- **Date:** 2026-11-12

## Context

A review of hand editing (ADR 0433, 0438, 0440–0442) found edits that left supports growth
never builds:

- A column on the model carried across slid whole (ADR 0441). Where its root is the neck its
  head hangs from — a single tip whose trunk drops onto the part — the neck slid with it and
  the head leaned off its face, the scar ADR 0339 removed.
- A joint giving way (ADR 0440) could be a neck, and moving it turned that head the same way.
- A tip carried by hand moved its neck to the new contact. Where another branch had come
  straight down onto that neck (ADR 0338), the neck was that branch's joint, and carrying one
  tip dragged the other branch with it.
- A support standing on another was checked for clearance only: carried, or seated by a host
  carried away, its strut into the host could lie flat. Growth seats one at the highest point
  within 45 degrees, so every host carried away from it flattened it.
- Freezing a support standing on a grown one left its host to the automatic run, which may
  regrow it elsewhere. Removing a frozen host, or growing it again, left what stood on it in
  the air (ADR 0433 named this as a consequence).
- `carried` pulled every tip onto its nearest surface, those the hand did not carry included.
- A tip carried onto a face looking up was refused as `Collides`.

## Decision

- **No edit moves the neck a head hangs from unless the hand holds that tip or neck.** A
  column on the model whose root holds a head is carried as a column on the plate is: on a knee
  its head leans down to at 45 degrees. A neck never gives way.
- **A tip carried off a shared neck takes a neck of its own**, hung from the old one, which is
  now a joint and may give way.
- **A trunk into a support leans no further than 45 degrees**, as growth builds it. A support
  seated on its host at the same share of the same axis that would lean further is seated
  instead where growth would seat it on that axis: the highest point within its branch lean,
  half its merge distance and its longest strut (`branch::meeting_on`).
- **Taking hold of a support freezes what it stands on**, down to the plate.
- **A frozen support standing on no other frozen support goes back to the automatic run**
  (`ModelSupports::thaw_unseated`), which the window asks after removing, regrowing or growing
  again a frozen one.
- **Only a carried tip is pulled back onto the surface.** A tip carried onto a face looking up
  is refused as `NoHold`.

## Consequences

Every head an edit leaves keeps the lean of the face it holds, and no support an edit leaves
stands in the air or leans onto another past 45 degrees. A host carried further than its
riders can follow within growth's reach is refused as `Leans`, where before they lay flat. A
touch freezes more: the hosts under what was taken, and those cannot regrow until **Grow these
again**. A support removed takes those standing on it back to growth, which may stand them
elsewhere. Reopen if carrying hosts is refused too often, or if frozen hosts pile up.

## Alternatives considered

### Refuse what would turn a head or flatten a rider, and change nothing else

Fewest changes, but grown riders sit exactly at 45 degrees, so almost every host carried away
from one is refused, and a column on the model whose head sits on its root could never move.

### Take away what stood on a removed support

Simple, but it removes tips the hand never asked to remove.

### Keeping heads and seats as growth builds them, and what it costs

A carried column on the model grows a knee rather than sliding whole, a rider moves up its
host as the host moves away, and a touch freezes the hosts under it.

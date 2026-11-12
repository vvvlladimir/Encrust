# 0438. Grow a carried small pillar again from its contact

- **Status:** Proposed
- **Date:** 2026-11-12

## Context

A small pillar is a support on the part with one tip and a short run (ADR 0255, 0340). A drag
carried its nodes as it carries any support's (ADR 0435). A tip held alone pulled its parent
to a regular head's neck, and a lone column stood again under it, on the plate. So a small
pillar became a long trunk on the plate. It kept its thin nodes and the bends that square it
off its faces. A small pillar grown from a contact with no room for a head became a strut
lying aside toward a trunk. A trunk or a foot carried across slid the column and pulled the
tip onto the nearest surface. That could be the face the pillar stood on, so it held nothing.

## Decision

- **A small pillar is carried by its contact.** Whatever part of it is held, the step carries
  its tip, and the support is grown again from there. `grown_alone` grows that one contact
  with `grow`, then falls back to the small pillar search (ADR 0340). The result is a small
  pillar, or a whole support where none fits, as a fresh Grow builds it.
- **A tip held alone** goes where the cursor points, onto the surface as before (ADR 0244).
- **A carried column's contact** is the nearest surface within 2 mm that looks down. Failing
  that, it is the first face looking down straight over it. With neither, the pillar is refused
  with `NoHold` and waits where it stood.
- **What is grown is judged as any carry**, by `allowed` (ADR 0435). A support grown again
  has no joints to give way, so it is not eased.

## Consequences

A small pillar dragged anywhere is either a small pillar or a whole support, and it holds a
face. A frame of such a drag costs one `grow` of one column: its landing, a soundness check
and, failing those, up to 50 rays of the small pillar search. A carried small pillar keeps
nothing of its old shape: hand-made bends are lost, and the contact may move as growing moves
it (ADR 0429). A small pillar carried under a higher overhang jumps up to it. Reopen if a
hand wants a small pillar to keep its shape, or a drag of many small pillars stutters.

## Alternatives considered

### Carry its nodes, then thin or widen the result

The bends that square its ends off its faces would be carried to where they no longer meet a
face square. A run past the longest would keep thin nodes with a regular body.

### Keep the pillar's foot and only aim it at the new contact

This is what other tools do with a two-part support. A foot left on a face under the old
contact leans the pillar past 45 degrees within a few millimetres, so most drags are refused.

### Growing it again, and what it costs

The drag shows a support that may change shape as it moves: a small pillar becomes a whole
support and back again across an edge of the face under it. Each frame grows one column.

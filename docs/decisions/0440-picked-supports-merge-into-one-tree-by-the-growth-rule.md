# 0440. Merge picked supports into one tree by the rule growth joins two fronts by

- **Status:** Proposed
- **Date:** 2026-11-12

## Context

A hand could pick several supports and send them to a group, grow them again or remove them,
but not make one support of them. Several thin trunks side by side print weaker and leave more
marks on the plate than one trunk carrying the same tips. Picked supports are frozen trees in
the model's space (ADR 0095); growth joins fronts pairwise at the highest point both reach at
45 degrees, and drops a refused pair twice as far each retry (ADR 0040, 0338).

## Decision

**Merge into one**, under Picked while two or more supports of one model are picked, builds one
frozen tree of them with `core_supports::merged`:

- Each tree keeps its tips, struts and heads; its root is a front, a tip standing straight on
  its trunk first given a neck one head-length under it.
- Fronts join two at a time, highest meeting point first, by growth's `meeting_point` at the
  heavier group's lean; a pair whose struts meet the model drops by growth's `lower` until it
  would meet under the plate, then is given up. Every joint holds two branches.
- One trunk stands from the last joint on what `footing_under` finds, built to the heaviest
  group; each head keeps its own.
- The trunk is checked, and the whole tree too when every picked tree stood clear before. A
  pair left apart is `Collides`, a joint with nowhere to stand `NoFooting`; the stage says so
  and the supports stay as they were.

The tree replaces the picked ones; what stood on them goes back to growth (ADR 0431, 0439).

## Consequences

One click turns a cluster into one support with no stray joint flatter than 45 degrees. A
merge cost grows with the square of the supports picked, as a heap of pairs, and one clearance
sweep per pair tried; a hand picks tens at most. A tree whose tips are far apart joins low and
may lean long struts; reopen if those print badly, by capping strut length as growth does.
The trunk carries every tip picked, past its group's `max_tips_per_trunk`: the hand chose
how many to join.

## Alternatives considered

### Give the tips back to growth, marked to grow on one trunk

Reuses all of growth, but a mark on points changes the `.encrust` format, growth may still
leave them apart, and the result shows only after the next rebuild.

### Joining the frozen trees, and what it costs

A second, smaller joining loop beside growth's `Forest`: the two share the meeting point and
the drop, not the heap, so a change to how growth picks pairs is not seen here.

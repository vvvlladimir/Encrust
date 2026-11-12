# 0433. Hold the end of a toe with a support of its own

- **Status:** Proposed
- **Date:** 2026-11-11

## Context

Placement samples a piece's corners, its rim a spacing apart and a lattice inside it
(ADR 0329), then drops every wanted cell that a standing support already holds within that
support's coverage radius, three quarters of a spacing. A toe narrower than the spacing —
the claws of a figure's foot on its first layer — gets one support at its root at most, and
the end of it, 1.5 to 2 mm further out, counts as held by that support. On `crustling` four
claws printed with nothing under their ends, and the window's check said nothing was wrong.

A support under a toe's root holds its end only as a lever: the end cures against the film
and peels off it before anything under it pulls.

## Decision

**A toe's end is a wanted cell of its own.** For each island, overhang, step and gentle
piece, `sample_ends` opens the piece by a disc of `end_reach` — a third of the spacing it is
sampled at — and takes what sticks out more than one and a half `end_reach` past that, which
a square's corner does not. Each such part's cell furthest from the piece's middle, inside
the room a head has, is wanted after the rest of the piece.

**It is held unless a support stands within `end_reach` of it**, not within the coverage
radius of the support beside it: `Held::holds` takes the smaller of the two.

Slopes a part is steadied on are left out: nothing hangs from them.

## Consequences

- On `crustling` as drawn, every claw end gets a contact: 154 → 162 contacts, 1.34 →
  1.41 ml of supports. The bench corpus is unchanged but for `t5-thin` (83 → 84 contacts).
- More contacts on figures with fingers, claws and spikes; each is an Auto Light tip unless
  strengthening says otherwise.
- Reopen if the ends' contacts leave scars where a toe is visible: Clean surface already
  spaces contacts out there, and could leave ends to it.

## Alternatives considered

### Hold every rim cell within half a spacing

The bench's `rim-gap` reads this way, and it would catch toes too; it puts supports along
every outline, roughly doubling the rim's contacts, for the few places that are toes.

### Pick each toe's end from the cells cut off by the opening alone

Tried first: the cells an opening cuts off join up along a ragged outline, so one toe and the
rim beside it were one part, and its end was taken somewhere on the rim.

### A support at every toe's end, and what it costs

A contact where a toe ends is a contact on a detail meant to be seen, and some ends are short
enough that the lever would have held.

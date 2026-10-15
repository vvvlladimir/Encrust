# 0280. Rank a pose set on the plate with the lifted ones, by one rule

- **Status:** Accepted
- **Date:** 2026-10-15

## Context

ADR 0277 put a pose that needs no supports ahead of every lifted pose, whatever the goal,
and a seat that needs some behind them all. ADR 0278 says one rule ranks every stage, and
ADR 0279 proves a seat with supports under what hangs. The two orders disagreed: without
a proof, the least peel took a bare seat over a lifted pose pulling half as hard, and a
seat needing a little support sat past the four to sixteen finalists the proof prints,
so the fewest supports never saw it. The search also stood every seat's support on the
lift, so its predicted volume could not be weighed against a lifted pose's.

## Decision

- **Seats and lifted poses are one list**, ranked by `core_plate::rank` on each goal's own
  reading; `Candidate::on_plate` tells them apart, and finalists are kept `apart_deg`
  apart only from poses held the same way.
- **`Score::lift_mm` is what the pose stands on**: the lift on supports, zero on the plate,
  and the predicted support volume takes it from there.
- **The proof always prints the best seat**, wherever it ranks, beside its finalists and
  the way the model stands.

## Consequences

On `orient-lab` without a raft: 76 goals of 92 pass against 75, none regressed; with a raft
72 of 89 against 75. The three that fail with a raft keep their choice: the lever's least
peel and clean top now meet a sound pose the fewest supports found, 0.05 ml upside down on
the plate against 0.24 ml, and a figure's least peel ran 3.0 s on a 3.0 s budget. The fewest
supports changes nowhere else. The least peel, the shortest print and the clean top now
take a seat where it reads as well as a lift: a puck, a bracket and a coupon print 100
layers shorter; a slab's clean top is lifted onto 1.9 ml, reading as well. Choosing takes
no longer. Reopen if a seat's predicted volume is found far
from what placement grows under it.

## Alternatives considered

### Print every seat in the proof

Up to seven more prints a model. Rejected: the best one is the one a goal can choose.

### Keep bare seats first for every goal but the fewest supports

Rejected: a goal that ranks by another goal's reading is not its own.

### The option that won, and what it costs

One more print where the best seat is not among the finalists, and a goal that ignores
the plate may lift a model a person drew to stand on it, where its reading is as good.

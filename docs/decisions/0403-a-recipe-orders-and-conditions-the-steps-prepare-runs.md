# 0403. A recipe orders and conditions the steps Prepare runs

- **Status:** Proposed
- **Date:** 2026-11-05

## Context

Prepare for print and `--prepare` chain repair, orientation, hollowing, drainage and
supports by fixed rules in a fixed order (ADR 0402). A user preparing many similar models
wants that chain to be theirs: steps on or off, their presets, thresholds of their own
("hollow only past 30 ml", "least peel above 120 mm"), a stop where a check fails, and the
file saved at the end. The window and the command line must run the same chain (rule 9 of
`architecture.md`); the decisions already live apart from the execution, in `core-engine`,
while each front end runs the steps with its own jobs or flags. `core-engine` takes nothing
that two callers merely share (ADR 0174).

## Decision

A **recipe** is a TOML file of steps, each with `when`, `set`, `choose` and `on_fail`, plus
`gate` and `pause` steps and a `[vars]` table, laid out in `docs/design/recipes.md`. The
autopilot of ADR 0402 is the built-in recipe `prepare`, so `--prepare` is `--recipe prepare`
and Prepare for print runs the recipe in hand.

A new crate, **`core-recipe`**, owns the format, the condition language, the order a recipe
may take, the variables measured on a model and `decide`, which answers a step `Run`, `Skip`,
`Stop` or `Pause` with what it read. It depends on `core-engine` for the measures, reads no
files and runs no step. The window and the command line depend on it and run each step as
they run it today.

Conditions are a small expression language parsed by `core-recipe` itself: comparisons,
`&&`, `||`, `!`, arithmetic and four functions over numbers and booleans, no loops. A step
follows what it works on, checked when the recipe is read; `repair` and `orient` take either
order. The one retry, `check` supporting islands again, is bounded at five.

## Consequences

Each new step kind is a variant in `core-recipe` and an executor in both front ends, in one
change. A recipe file is a format: its `format` number moves when a key changes meaning. A
condition reads only what is measured; a variable no step can measure is not offered. The
order is not free: what `hollow` makes, `drain` and `supports` need, and the file says so
when read rather than mid-run. Reopen the language if users ask for strings or loops.

## Alternatives considered

### The chain in `core-engine`

It already holds the autopilot's decisions. Rejected: a format, a parser and a validator are
not running a plate, and ADR 0174 keeps `core-engine` to that.

### An expression crate (`evalexpr`) or a script (Rhai, Lua)

Less code to write. Rejected: a new dependency for a grammar of a dozen rules, and a script
can loop, read files or never end; a recipe must always end and be checked before it runs.

### The order entirely free

What a user asked for at first. Rejected: draining before hollowing or supporting before
orienting builds nothing usable, so the only freedom left is between repair and orient.

### The decision that won, and what it costs

A fourth layer between the user and the tools: a recipe that misjudges a model prepares a
whole batch badly at once. The plan shown before a run and `pause` exist for that. Two
front ends still execute every step, as they did for ADR 0402.

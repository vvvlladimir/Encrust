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
they run it today: the window by each tool's own job, the command line step by step over the
plan, each kind by the call its flag makes, so `--prepare` is no longer a resolution of flags
into the one pass of ADR 0402. There a flag typed, or a plate file's key, wins over the
setting it shapes in every step of its kind; it never overrides a condition.

Conditions are a small expression language parsed by `core-recipe` itself: comparisons,
`&&`, `||`, `!`, arithmetic and four functions over numbers and booleans, no loops. Steps
take any order and may repeat: each works on the model as the steps before left it, a step
with nothing to work on is skipped with why, and a step that undoes an earlier one is noted
in the plan, never refused. The one retry, `check` supporting islands again, is bounded at
five.

## Consequences

`--prepare --hollow 3` on a model too thin to save resin now prints it solid, where ADR 0402
hollowed it: the typed wall is a setting and the step's condition still decides.

Each new step kind is a variant in `core-recipe` and an executor in both front ends, in one
change. A recipe file is a format: its `format` number moves when a key changes meaning. A
condition reads only what is measured; a variable no step can measure is not offered. A free
order lets a recipe build something useless, supports before the turn that drops them; the
plan's notes say so before the run, and each pair that undoes work needs its row in the
design note. Reopen the language if users ask for strings or loops.

## Alternatives considered

### The chain in `core-engine`

It already holds the autopilot's decisions. Rejected: a format, a parser and a validator are
not running a plate, and ADR 0174 keeps `core-engine` to that.

### An expression crate (`evalexpr`) or a script (Rhai, Lua)

Less code to write. Rejected: a new dependency for a grammar of a dozen rules, and a script
can loop, read files or never end; a recipe must always end and be checked before it runs.

### An order fixed by what each step needs

The first draft refused draining before hollowing and supporting before orienting. Rejected:
a hole drilled before the cavity is a recess the user may want (ADR 0346), and what is
useless elsewhere is better said than forbidden.

### The decision that won, and what it costs

A fourth layer between the user and the tools: a recipe that misjudges a model prepares a
whole batch badly at once. The plan shown before a run and `pause` exist for that. Two
front ends still execute every step, as they did for ADR 0402.

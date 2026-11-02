# 0397. Prepare a model by the engine's defaults, filling in the flags left out

- **Status:** Proposed
- **Date:** 2026-11-02

## Context

Preparing a model for print takes six flags chosen by judgement: whether to repair, how to
turn it, whether and how thick to hollow, where to drain, what to stand it on. A language
model driving the command line (ADR 0395) needs a baseline to be measured against, and a
user with no model needs the same answer without one (`docs/design/ai-integration.md`, step
4). Each piece exists: the import diagnosis, `--orient-goal auto`, the saving of ADR 0393,
`--drain-at auto` (ADR 0396), `--supports auto` (ADR 0325) and `--support-cavity` (ADR 0332).
What is missing is the rule that chains them, and the window must be able to run the same
rule (rule 9 of `architecture.md`).

## Decision

The decisions are `core_engine`'s, plain functions over plain values: `needs_repair` repairs
a model whose diagnosis is not sound, by `PREPARE_REPAIR`, the window's full mend;
`PREPARE_ORIENT` searches every goal at once; `hollowing` hollows a sound model at Hollow's
default 2 mm wall only where ADR 0393 names a saving, its cavity left empty. A hollowed
model is drained by `suggest_drains` with a vent, and every model stands on the catalogue's
automatic supports, a hollowed one's cavity held up too.

The command line takes it as `--prepare` on `slice`, `estimate`, `render` and `batch`, as a
resolution of the flags, not a second pipeline: before import each model left without
supports gets `auto`, and one no `--rotate` or `--orient` turned is oriented; at import an
unsound model is repaired; after it, the wall, `--drain-at auto` and the cavity's supports
are set on its `Shaping`, and the run stages as if they had been typed. A flag typed, or a
plate file's own key, wins over its step; a flag shaping a step not taken is set aside.
`--strict` is not implied. A project refuses it. `--json` gives each model `prepared`:
`repaired`, `oriented`, `hollowed` (`wall_mm`, `saves_mm3`) or `null`, `drained` and
`supports`.

No infill: it is the window's default, a lattice can seal off a pocket the drain search
cannot see, and it pushes the search's holes down its ranking (ADR 0396); a 2 mm wall on a
model saving at least 10 ml stands without one.

## Consequences

An agent's first call can be `--prepare` and its later ones the flags it disagrees with;
the report says what to disagree with. The autopilot costs what its steps cost: up to ten
slices for the drains, and an orientation search. The model is turned before it is
repaired, as `--orient --repair` already does, so a broken model is searched as it came.
The window has no single button yet: it would chain its Repair, Auto-orient, Hollow and
Supports jobs, and its Drain tool has no automatic search (ADR 0396), so a "Prepare for print"
action is the next step once Drain offers one, calling these functions. Reopen if users
want infill or a thicker wall by size, or if repairing before orienting proves better.

## Alternatives considered

### A `prepare` subcommand with its own pipeline

Clear to call, but it would stage a model a second way beside `slice`, and every flag a
user wants to override would have to be repeated on it.

### The decisions in the command line

Fewer crates touched, but the window could then only copy them, and two copies of "worth
hollowing" drift.

### Defaults resolved before import

Simpler, but the hollowing decision needs the model as it will print, turned and scaled, which
only import knows.

### Filling in flags, and what it costs

A step's decision is spread over three moments of staging, and `ImportArgs` carries a field
no flag sets so placement and repair can tell a prepared model from one that is not.

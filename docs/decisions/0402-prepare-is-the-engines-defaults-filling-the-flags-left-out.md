# 0402. Prepare a model by the engine's defaults, filling in the flags left out

- **Status:** Proposed
- **Date:** 2026-11-02

## Context

Preparing a model for print takes six flags chosen by judgement: whether to repair, how to
turn it, whether and how thick to hollow, where to drain, what to stand it on. A script or a
user who tuned nothing needs one answer for all six. Each piece exists: the import diagnosis, `--orient-goal auto`, the saving of ADR 0393,
`--drain-at auto` (ADR 0401), `--supports auto` (ADR 0325) and `--support-cavity` (ADR 0332).
What is missing is the rule that chains them, and the window must be able to run the same
rule (rule 9 of `architecture.md`).

## Decision

The decisions are `core_engine`'s, plain functions over plain values: `needs_repair` repairs
a model whose diagnosis is not sound, by `PREPARE_REPAIR`, the window's full mend;
`PREPARE_ORIENT` searches every goal at once; `hollowing` hollows a sound model at Hollow's
default 2 mm wall only where ADR 0393 names a saving, its cavity left empty. A hollowed
model is drained by `suggest_drains` with a vent, and every model stands on the catalogue's
automatic supports, a hollowed one's cavity held up too.

The command line takes it as `--prepare` on `slice`, `estimate` and `batch`, as a
resolution of the flags, not a second pipeline: before import each model left without
supports gets `auto`, and one no `--rotate` or `--orient` turned is oriented; at import an
unsound model is repaired; after it, the wall, `--drain-at auto` and the cavity's supports
are set on its `Shaping`, and the run stages as if they had been typed. A flag typed, or a
plate file's own key, wins over its step; a flag shaping a step not taken is set aside.
`--strict` is not implied. A project refuses it. `--json` gives each model `prepared`:
`repaired`, `oriented`, `hollowed` (`wall_mm`, `saves_mm3`) or `null`, `drained` and
`supports`.

The window takes it as Prepare for print, in the Select tool under Auto-orient and in the
Edit menu, on the picked models or the whole plate. It starts each tool's own job in turn,
on those models and with these decisions rather than the panels' values, in the command
line's order: Auto-orient by `PREPARE_ORIENT` on the automatic supports for a model whose
turn is nought, which is the window's `--rotate` left out; the full mend where the
diagnosis is not sound; `hollowing` read off the repaired and posed model; Drain's Auto;
the catalogue's automatic supports outside, then in the cavity. The run owns each step's
job, so no tool's own Cancel or start reaches it, and the tools' own starts wait, saying why,
while it runs; it begins only once no other job that lands on the plate is running. A step
starts once the last one's job has landed and its cuts and support surface are built, waiting
on frames with a button down as the window's cutting does; a step that fails or stops puts the
plate and the tool values back and leaves its reason. What the run did to a model is read off
what each step changed: a turn, a new mesh, more holes, a cavity, supports. A model already hollow keeps its own cavity: it is drained
and held up inside, and is not said to be hollowed by the run. Select then lists for the
model in its card what `prepared` would say, while it stands as the run left it.

In the window a prepare is one edit, though its jobs land over many frames:
`History::hold` keeps every change from the stack until `release` records them as one entry,
and `abandon`, which Cancel calls, puts the plate back as the hold found it and records
nothing. This carries ADR 0257 from one frame to a run of them: what each landing sets off,
a model settling onto its raft or the plugs for its holes, falls inside the hold, and an undo
of the entry settles the plate as any undo does. Undo and redo wait while a hold runs.

No infill: it is the window's default, a lattice can seal off a pocket the drain search
cannot see, and it pushes the search's holes down its ranking (ADR 0401); a 2 mm wall on a
model saving at least 10 ml stands without one.

## Consequences

An agent's first call can be `--prepare` and its later ones the flags it disagrees with;
the report says what to disagree with. The autopilot costs what its steps cost: up to ten
slices for the drains, and an orientation search. The model is turned before it is
repaired, as `--orient --repair` already does, so a broken model is searched as it came; the
window keeps that order so both make the same decisions. Reopen if users want infill or a
thicker wall by size, or if repairing before orienting proves better.

An edit the user makes while the window prepares falls in its entry, and Cancel takes it
back with the rest. In a browser a worker that panics never answers, so the step it ran stays
under way until Cancel.

## Alternatives considered

### Merging the steps' entries once the run ends

No new state in the history, but an entry recorded between two landings may be the user's,
and Cancel would then have to undo a count of entries that a tool value changed meanwhile
alters.

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

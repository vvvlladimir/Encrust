# 0478. The command line and the agents' server reach the window's model work

- **Status:** Proposed; a plate's objects by id (0479)
- **Date:** 2026-11-25

## Context

The window places, shapes and checks a model in ways neither front end without it could ask
for: a joint sized or a cut left upright, a mirror, a move or copies, the print settings of
one plate, Check's automatic answers, several models on one plate. `encrust-cli` lacked the
flags; `encrust-mcp` lacked even some the command line had — the lift, a clean surface, thin
parts kept solid, a label's font, the weld. An agent asked to print two models had no way to;
a cut it asked to arrange reached the window as a cut alone. The architecture rule holds the
command line to the window's pace (`.claude/rules/architecture.md`, 9), and every action is
already an `Edit` the document applies (ADR 0466).

## Decision

- **A flag for each, the same edit or step as the window.** `--joint-count`,
  `--joint-clearance`, `--peg-size`, `--magnet-size`, `--key-size`, `--unkeyed` and
  `--cut-upright` size the `Cut` edit through `Joints::asked` in `core-joint`, one home for
  both front ends; `--mirror`, `--move` and `--copies` are the `Mirror`, `Move` and
  `Duplicate` edits, copies made once the plate is finished, as the window copies a prepared
  model. The exposure, motion, waits and light power are typed over the resin for one run
  (`--exposure` and the flags beside it), as Print settings change the plate in hand.
- **Check's answers run as a recipe's `check` step**, appended last: `--support-islands`,
  `--drill-cups`, `--hold-necks`. A run with no recipe takes the flags' own steps
  (`--recipe-out`'s) before it, so the answers have one implementation, the executor's.
- **The server's tools carry every one of those flags.** A field changed that an edit carries
  — a mirror, a move, a cut's joint or pose, a font, thin parts kept solid, a clean surface —
  reaches the window as that edit; one no edit carries — weld, lift, copies, Check's answers —
  is checked by its flags and shown by its plate. `fix_issues` sets Check's answers; numbers in
  `set_slicing`'s `print` only as the user or the resin's maker gave them.
- **Several jobs make one plate, not one job several models.** `encrust` lays several projects
  on one plate, each the plate it was left on (`core_engine::project::insert`, moved down
  from the window, which now shares its regrouping), and `--arrange` spreads them; the server's
  `plate_jobs` lays each job's latest revision so, on one printer and resin, checks it and
  writes one file.

## Consequences

A chat client and a script reach what the window does to a model, short of painting, blocking
areas from supports and moving one support. A job stays one model, so every edit names one
object and the window link is unchanged. A plate of several jobs is checked as a whole only
by `plate_jobs`; its findings are not any one job's revision. Reopen if an agent needs to
edit models together after they are on one plate.

## Alternatives considered

### Several models in one job

Closer to the window's plate — but every tool would name a model, the recipe, the edits and
the window link would be rebuilt, and the window shows a job on a plate of its own (ADR 0477).

### Check's answers as edits of their own

Lands them in the window as edits — but each needs the slice the check measured, which the
document does not hold; the recipe executor already measures and answers.

### The option that won, and what it costs

More flags and more tool fields to keep in step with the window, a fixed copy gap and row
before `--arrange`, and slicing settings taken from the first job of a `plate_jobs` plate.

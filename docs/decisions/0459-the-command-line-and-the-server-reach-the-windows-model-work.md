# 0459. The command line and the server reach the window's model work

- **Status:** Accepted
- **Date:** 2026-11-03

## Context

The window places supports by a click, paints the side meant to be seen, sizes a joint, leaves
a cut upright, mirrors, moves and copies a model, sets one plate's print settings, answers
Check's findings and holds several models on one plate. The command line had none of these, so
neither a script nor a chat client over `encrust-mcp` (ADR 0455) could ask for them; rule 9 of
`architecture.md` holds the command line to the window. Every point the command line takes is
plate mm as the model prints (ADR 0400).

## Decision

- **Supports by hand:** `--support-at X,Y,Z` adds a sturdy contact on the nearest surface,
  `--support-only-at` with none automatic.
- **What shows:** `--shown-at X,Y,Z[:R]` (4 mm) paints the faces the window's brush reaches,
  where supports stand sparser and lightest (ADR 0425); the server's `shown_at_mm`.
- **The window's model work as flags**, each the same edit: `--joint-count`,
  `--joint-clearance`, `--peg-size`, `--magnet-size`, `--key-size`, `--unkeyed` and
  `--cut-upright` size `Cut` through `core_joint::Joints::asked`; `--mirror`, `--move`,
  `--copies`; `--exposure` and the flags beside it over the resin for one run.
- **Check's answers** — `--support-islands`, `--drill-cups`, `--hold-necks` — run as a recipe's
  `check` step, appended last; the server's `fix_issues`.
- **Several jobs make one plate**: `encrust` lays several projects on one plate
  (`core_engine::project::insert`, moved down from the window), and `plate_jobs` checks and
  writes it.
- **The server's tools carry every flag**; one an edit carries reaches the window as that edit.

## Consequences

A chat client and a script reach what the window does to a model, short of blocking areas and
moving one support. Two ways to keep supports off remain — places kept clear for the pose
search (ADR 0460), painted for the supports. A plate of several jobs is checked as a whole only
by `plate_jobs`. Reopen if an agent must edit models together once they share a plate.

## Alternatives considered

### A plate file listing supports

It carries a model's settings already — but a point list is what a flag holds as well.

### Check's answers as edits of their own

Each needs the slice the check measured, which the document does not hold.

### The option that won, and what it costs

More flags and tool fields to keep in step with the window, one steadying shape for every hand
support, and slicing settings taken from the first job of a `plate_jobs` plate.

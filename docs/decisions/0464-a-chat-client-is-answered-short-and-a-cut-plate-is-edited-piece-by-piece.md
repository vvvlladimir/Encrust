# 0464. Answer a chat client short, hand its points to their objects, and edit a cut plate piece by piece

- **Status:** Accepted; `fix_issues` refused once points name objects is superseded by 0465,
  the edits of a kept plate with no window by 0467
- **Date:** 2026-11-27

## Context

A session cut a 1.4-million-face statue in two, then posed, hollowed, drained and supported the
halves: about ten minutes, of which the command line ran about three. Each answer carried every
risk the check found, 95 islands with their places, so the client reread tens of thousands of
tokens a step. A cut plate is no plate of one model, so ADR 0456's edits on the kept plate never
applied after it: every step staged the file again, cut, pose, hollow and all (ADR 0455's
cost), and each step took longer than the one before. A point given without `objects` on two
halves was refused by the command line (ADR 0458), and each refusal cost a full stage. A report's
`seconds` counts its last run alone, so a step of several runs looked faster than it was.

## Decision

- **An answer lists at most twelve risks**: every kind but an island in the check's order, then
  the largest islands, with `risks_left_out`; the counts stay whole. `check` with `every_risk`
  lists them all, and `N.json` keeps them as before. Every answer carries `took_s`, the whole
  call's time, every run in it.
- **A point given with no `objects`, several objects on the plate**, goes to the object whose box
  in the latest check holds it or lies nearest; the step stands for every object without the
  points, then once for each object with its own (`more_*`, ADR 0458). A point as near two boxes
  is refused, naming both. Drain points, raft holes, channels and merges by their first point,
  supports, what shows and walls kept solid are handed out alike.
- **A revision whose plate is in pieces is edited on the plate it kept**: each piece's pose, hollow,
  holes and supports beyond the revision before become edits of that piece (the step naming it
  last, else the one for every object), the spread after them where the recipe arranges. A change
  to the pieces themselves — the modelling, a repair, a relief, the lift, Check's answers — or
  one no edit carries yet is staged by the flags as before. Such a revision keeps its plate and
  edits (`N.encrust`, `N.edits.json`) and is shown to an open window as its plate; a plate of
  pieces is never edited as the job's one model.
- **The command line's refusal of a drain point off an edge or corner** says where the hole
  would sit and what to do instead.

## Consequences

A step after a cut costs that step and a check, not the stage of everything before it; an
answer costs the client a few hundred tokens of risks, not thousands. Points handed out make a
recipe name objects, which keeps `fix_issues` from running on it: Check's answers run through a
recipe, which the command line refuses on a project (ADR 0459), and each run after the first is
on one. Reopen when the command line runs Check's answers on a project, or when an answer of
twelve risks leaves a client unable to act on one it was not shown.

## Alternatives considered

### Keep the plate of each run of a staged recipe, and stage from the longest prefix kept

Reaches every recipe, the window's plate untouched — but a step that changes nothing but the
last run still checks the plate after each run before it the first time, and the job folder,
a format two binaries share (ADR 0455), keeps a plate a run.

### Let `--only` be inferred by the command line from where each point lies

One rule for every front end — but the command line's flags apply to every target alike, and
splitting each pointing flag by object reaches every finishing step.

### The option that won, and what it costs

The edits of a piece repeat what `asked_after_check` does for one model, piece by piece; a plate
of pieces the window holds is shown, never edited in place, so what the user did there after the
cut is not in the next check; and a point near where two pieces' boxes meet asks for `objects`.

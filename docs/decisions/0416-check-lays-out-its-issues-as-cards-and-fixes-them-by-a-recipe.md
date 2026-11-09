# 0416. Check lays out its issues as cards and fixes them all by a built-in recipe

- **Status:** Accepted
- **Date:** 2026-11-09

## Context

ADR 0415 gave each kind of issue a button, but they stood in sections above a separate list,
so what was wrong and what answered it were apart. Answering everything still took one press
per kind, each with its own undo, and the order mattered: a model moved after its supports
stood, or a hole drilled after the layers were read, leaves the next answer stale.

## Decision

Check has one card per kind of issue, holding its rows and its buttons, one card open at a
time. Fix issues is `assets/recipes/fix-issues.toml`, a built-in recipe run on the whole plate
by Prepare's machinery: clear the way, read, drain what stays trapped, then drill cups, hold
necks and support islands. What a fix did is noted by the kind and the undo edit that holds
it (`check::Answers`), said on the card until a later edit that is no fix replaces it.

## Consequences

Fix issues is one undo, cancels by putting the plate back, and is `--recipe fix-issues` on the
command line with no second orchestration. It reads the layers twice, so it costs one slice and
measure more than the sum of its buttons. It lists among the recipes Prepare for print offers.
Undo on a card stands only on the newest answer, as the history is linear. A kind whose fix has
had its turn drops that button and counts as left to the hand. Hard peel and thin sections stay
out: a pose chosen for peel would undo the supports and holes the rest just made.

## Alternatives considered

### A sequencer inside the Check panel

It would have to wait on the same jobs Prepare waits on, repeat its hold on the history and
exist again in the command line.

### One card per layer

A stack with many islands would scroll, and the answers are per kind, not per layer.

### The option that won, and what it costs

A recipe is visible in the picker and has to stay valid as a recipe, and a retry says only what
its first pass stood.

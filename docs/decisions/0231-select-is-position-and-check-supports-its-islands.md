# 0231. Fold Select into Position, and let Check support the islands it finds

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

ADR 0218 gave the rail a Select tool that only said what was picked, beside a Position tool
that picked as well and did everything else to the selection. Two entries did one job, and
the first digit went to the one that changed nothing. Check's "Support every island" only
opened Supports, which grows supports under every overhang of the plate rather than under
the islands the check had just named. The last message stood under the running jobs at the
top of the stage, over the model being looked at.

## Decision

- **Select is gone.** `Tool::Position` takes its place at the head of the rail under the
  name Select and its glyph. With nothing picked it says how to pick and offers Select all;
  a model reaching past the volume is named there with the move that brings it back.
- **The digits follow the rail:** 1 is Select, 9 Shape, 0 Check; Export and the print
  settings have no key.
- **Support every island stands one support under each island and each part hanging over
  a removed one**, in a support group named Islands, made the first time from the numbers
  in hand and joined on every later press. The island's centre is taken back from panel
  millimetres to the plate through the layout the check measured on (`preview::as_seen`), and a ray up from a layer under the
  island's first layer finds its underside; an island whose centre misses it is left to
  Paint.
- **The last message stands at the foot of the stage**, between the side cards and clear
  of the view tools; the jobs and a sent file stay at the top.

This supersedes ADR 0218's Select tool and its digits, and ADR 0217's message under the jobs.

## Consequences

One press of 1 picks and places. Check fixes what it reports without the user aiming a
brush, and the supports it adds stay apart in their own group, so they can be retuned or
taken away together.

A crescent or a ring whose centre is empty gets no support and is still reported; the
reopen signal is a user pressing the button twice and the same island staying.

## Alternatives considered

### Run the automatic placement from Check

Rejected: it supports the whole plate by overhang angle, which is a different question from
"what will fail", and adds supports where the check found nothing wrong.

### Keep Select and drop Position

Rejected: the gizmo and the transform fields would move to a tool named for picking, which
is where ADR 0218 took them from.

### The decision above, and what it costs

The tool is `Tool::Position` in the code and Select on the screen, which a reader has to be
told once. One support at a centre holds a small island and may not hold a wide one; the
group's profile, not the button, decides how thick it is.

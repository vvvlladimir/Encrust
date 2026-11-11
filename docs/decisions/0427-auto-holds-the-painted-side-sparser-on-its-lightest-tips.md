# 0427. Hold the side painted to be seen sparser, on Auto's lightest tips

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

ADR 0409 reads what shows of a model, and ADR 0410 lets the user paint it with the Clean
surface brush. Both serve the orientation search alone. Its step 3, support placement within
a pose, was left open: a contact on the painted face still took the spacing and the size it
would take anywhere else.

Every contact leaves a mark. How large depends on the tip, and how many on the spacing. Auto
sizes a tip by its share of the model above it (ADR 0424) and strengthens a tip whose pull
margin falls under ×1.5 by making it heavier (ADR 0422). Both make the marks on a face larger.

The brush sat in Position, under Auto-orient, behind the Clean surface switch, where nobody
looking at supports would find it.

## Decision

- **Placement reads the painted faces as `core_supports::Shown`,** a patch beside `Blocked`. A
  contact within its head and clearance of the patch is on it.
- **Contacts on it stand 1.3 times as far apart.** Each wanted cell there keeps the next one
  away as a cell of 1.3 times its own spacing would.
- **A tip on it is Auto's lightest,** whatever its share asks: `Need::shown` sizes it to 0.
  A steadying support keeps its own shape.
- **Strengthening gives it a neighbour, never a heavier tip.**
- Only what is painted counts. What shows by the shape alone still steers the orientation
  search and nothing else.
- **The Clean surface switch and its brush stand in Auto-orient and in the Where section of
  Supports.** It is one switch, and both brushes paint the same faces. Placement reads the
  painted faces only while it is on.

## Consequences

`support-lab --shown-from 0.5` paints every face at least half as prominent as the most
open detail. On the bench's five figures there are 11 to 34 % fewer contacts and up to 1.0 ml
less resin per figure. The weakest pull margin stays at ×2.2 or over. Bodies with nothing
painted are unchanged (`supports-bench/2026-11-11-shown/`).

By the area rule (docs/design/analysis.md, "Calibration") the figures now load their tips
harder. Their upper tenth reaches up to 1.8 of what a tip holds, where the coupon's plates
swayed at 1.5. Strengthening reads only the pull, so it does not answer this. A painted face
that sags, or a tip that tears on one, reopens the factor.

The command line cannot paint. A project that carries painted faces keeps the supports
grown in the window, as it does for blocked faces.

## Alternatives considered

### 1.15 times as far apart

5 to 16 % fewer contacts, the upper tenth up to 1.42 of the area rule. Rejected: half of the
gain, and still past 1.2.

### 1.3 times, strengthened by the area rule as well

A neighbour wherever a tip on the face carries past 1.2 of the area rule would hold the gain
where the face can take it. Not taken now. It adds a second criterion to strengthening, and
the pull margin is the one the stage-three print confirmed.

### No contact on the painted face at all

The cleanest face, but a face that hangs and is painted would print unheld.

### The option that won, and what it costs

Tips on the painted face carry more than the coupon held flat. The pull reading says they
hold, and the area rule says they are near the limit.

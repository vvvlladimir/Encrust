# 0350. Turn a head lying past 45 degrees back down, by at most 33, behind a pin

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

ADR 0346 sent every head out along its face's normal, whatever way that looks. Off a wall
or a steep face that normal is nearly level, so a head ran out sideways one `[top]` long and
the trunk bent down under it: a hook users read as a fault. The head's mark is why it did
not lean: the cone behind the tip's half ball (ADR 0345) widens from its equator, and a
face tilted off the head's axis cuts that cone too, its mark reaching
`r / (cos θ − sin θ · tan α)` on one side, where `α` is the cone's half angle. For the
automatic Medium head that is 6 per cent at 10 degrees, 33 at 30 and 82 at 45. Turning
every head down, gentle faces included, bent supports users had found straight.

## Decision

A head leaves its face square on whenever that is within 45 degrees of straight down
(`STEEPEST_SQUARE_HEAD_DEG`), the lean ADR 0249 holds a strut to. Past it the head lies
over, and is turned back towards straight down to 45 degrees, by at most 33
(`MAX_HEAD_LEAN_DEG`): a wall's head leaves at 57 degrees off vertical. Behind the half
ball a turned head runs at the contact's width for `r · tan θ` along its axis before it
widens (`pin_mm`), where that width leaves the face's plane, so the face cuts the ball and
the pin only: the mark is the contact's circle on one side and reaches `r / cos θ` on the
other, its width growing by `(1 + 1/cos θ) / 2`, 9 per cent at 33 degrees. The head is swept
for clearance from the end of its pin. Not placing a support whose head meets the model,
and the rest of ADR 0346, stand.

This supersedes ADR 0346's head along the face's normal whatever way it looks.

## Consequences

Heads under faces within 45 degrees of level are as before. Off steeper faces the trunk
bends less, and the mark is up to 9 per cent wider than its contact, on one side. The pin is
at most 0.1 mm long on the shipped profiles. A head whose square-on sweep meets the model is
still left out rather than turned for room. Reopen if marks show on finished prints, or if
steep faces still grow hooks users complain of.

## Alternatives considered

### Turn every head down by up to 33 degrees

Tried first: gentle overhangs lost their square-on heads and supports looked crooked again.

### Keep the head square on and only shorten it off steep faces

The mark stays exactly the contact's, but the trunk then stands as close to the wall as
the head is short, and more contacts would have no room to stand.

### Lean the head with no pin

The cone's own mark is a third wider at 30 degrees: the slantwise scar ADR 0346 removed.

### The option that won, and what it costs

Off steep faces the mark is no longer exactly the contact's width, which ADR 0346 promised;
45 and 33 degrees are constants no profile sets.

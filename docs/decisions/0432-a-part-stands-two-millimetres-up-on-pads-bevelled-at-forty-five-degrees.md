# 0432. Lift a part 2 mm, on pads bevelled at forty-five degrees

- **Status:** Proposed
- **Date:** 2026-11-11

## Context

ADR 0237 lifted every shipped preset 5 mm so that a pad, a flare, a whole top segment and
the shortest pillar fit under the lowest contact. Printed, that is 3 mm of supports and
60 layers at 0.05 mm under every part for a neck that only the lowest contacts need whole.

ADR 0130 stood a straight pad's wall 59 degrees off the plate (`FOOT_BEVEL` 0.6 as the
tangent of its lean). The wedge a blade goes under is the angle between plate and wall;
at 59 degrees it is blunter than the 45 degrees the tapering foot's bevel already has.

## Decision

**Every shipped preset lifts a part 2 mm.** What a preset guarantees under its lowest
contact is its pad, its flare and `MIN_PILLAR_HEIGHT_MM`; a contact lower than that plus
its top segment keeps a neck shortened to the room, as the column already does.

**Every pad meets the plate at 45 degrees.** A straight pad's wall pulls in as far as it
drops, over its whole height: `foot_bevel_mm` is the pad's height. A tapering pad keeps its
rim `FOOT_BEVEL` (0.6) of its height up and its 45-degree bevel under it, as before. The
mesher, the pad's fitting and the Supports window's drawing all read `foot_bevel_mm`.

## Consequences

- A supported part prints 3 mm shorter; supports under the lowest few contacts are short
  enough to have a cut neck, and trees there have less room to merge.
- A straight pad meets the plate on a smaller sole: a 5.5 mm pad 0.5 mm thick on a 4.5 mm
  ring instead of 4.9 mm, about 84% of the area it had. Grip is traded for release again.
- Reopen the lift if low necks snap on the peel; reopen the bevel if parts drop off the
  plate.

## Alternatives considered

### Keep 5 mm and shorten only the top segment

Keeps whole necks under every contact, and keeps the 3 mm of supports this decision was
taken to remove.

### A bevel field in the profile

Lets each user choose the wedge. The angle is the decision, as for the flare (ADR 0130);
a field is a number nobody has asked to set.

### 2 mm and 45 degrees, and what they cost

Low contacts lose the full length of their neck, which is what reaches in under a ledge,
and every straight pad holds the plate on a smaller sole.

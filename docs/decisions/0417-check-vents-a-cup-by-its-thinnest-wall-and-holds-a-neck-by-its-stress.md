# 0417. Vent a cup through its thinnest wall, hold a neck by its stress, name a cup by its pull

- **Status:** Accepted
- **Date:** 2026-11-09

## Context

Check's Drill them (ADR 0415) drilled straight down from a cup's centre through its floor and
refused when the floor was the plate or stood on it. A model printed flat on the plate with a
recess in its base is the commonest cup there is, and Drill them did nothing for it. Usual
practice in resin printing is a vent at the cavity's lowest point by the shortest way out, a
channel cut in the base for a part that stands on the plate, or a pose without the cup.

Support them stood one support under the piece pulling on a weak neck, with the panel's tip.
A 0.3 mm tip or a small pillar in its place adds next to nothing to the neck, so Check's next
reading found the same neck weak.

A cup was named from 25 mm², about 2.5 N at a full vacuum. On a model held by the plate that
is nothing beside the lift, and a 27 mm² pocket was listed as an issue the user could not act
on.

## Decision

A vent starts at the cup's lowest point, half the hole sunk into the floor, and goes out along
the shortest of 17 ways: straight down, down at 45° and level, eight ways round each. A ray
is out of the wall once it has left every shell it entered. The thinnest wall up to 15 mm
wins. A level vent out of a cup on the plate is a notch along it. A vent going down is refused
where the plate lies under its rim. With no way out, `Unvented` says why: `OnThePlate` when
the floor is the plate or stands on it, and the notice tells the user to lift the model onto
supports or turn it, or `TooDeep`.

A weak neck gets as many supports as `holds_for_lever` counts. It reads the neck as the
analysis does, each tip's area added and the centre drawn towards the piece. It stops at the
fewest that bring the stress under 0.8 of the limit, at most 16. The tip and small pillar
are at least 0.6 mm across, and the supports stand in their own *Weak necks* group.

A cup is named when a full vacuum in it pulls at least `CUP_LIMIT_N`, 10 N: a third of
`PEEL_LIMIT_N`. Auto-orient counts cups by the same threshold.

## Consequences

Fix all now vents the plate-standing cup that used to be left to the hand, and a neck it
holds is no longer found weak the next time. The CLI's `drill_cups` and `hold_levers` do the
same through the same calls. A vent can now come out of a side wall: a visible hole on a
surface that a straight-down vent would have hidden underneath. Auto-orient may now accept a
pose that leaves a small pocket a vent no longer reaches; the ball test bounds that by the
cup threshold. Cups between about 25 and 99 mm² are no longer reported, including ones in a
thin wall that could blow out. Reopen this if prints fail at cups that are no longer named.

## Alternatives considered

### Keep drilling straight down and only say why it failed

This keeps every vent out of sight underneath. It answers none of the cups that stand on the
plate, and those are the ones a user meets most.

### Weigh a cup's suction into the pull of the piece it sits in

This would give necks and peel the cup's own share of the pull, which is better physics. The
cups are found per layer as holes, apart from the pieces, so it reworks the fold. The force
threshold catches the cups that matter until that is worth doing.

### The option that won, and what it costs

17 directions miss a thinner wall at an angle between them, and 15 mm and 10 N are warning
levels chosen by judgement, not measured. A notch along the plate cuts the model's footprint,
and the bottom layers' spread can narrow it. A neck held by 16 supports may still read weak;
it is then left to the hand.

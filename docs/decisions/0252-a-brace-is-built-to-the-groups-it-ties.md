# 0252. Build a brace to the groups of the trunks it ties

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

The bracing was the plate's, built to group 0's profile with the raft (ADR 0094): whether
any trunk was tied, how far apart, how thick the brace and from what height all came from
the first group, and every brace was meshed and drawn with it. Since a profile chosen from
the list became a group of its own (ADR 0247) a part carries several groups as a matter of
course, and switching bracing on for the group in hand did nothing unless that group was
the first; with it on there, the braces between trunks of another group still came out in
the first group's width and colour.

## Decision

`[bracing]` is each group's own.

- A trunk is tied only where its own group has bracing switched on, and is tall enough by
  its own group's start height and rise.
- Two trunks of one group are tied by that group's numbers.
- Two trunks of different groups are tied where both groups brace and they stand no
  further apart than both let a brace span. The brace is built to the finer of the two:
  the group whose brace is the thinner gives the diameter, the start height and the rise,
  and the lower-numbered of two alike.
- A brace is meshed with the group it is built to, so it is drawn, hidden and counted
  with that group. The lean towards a neighbour (ADR 0250) is asked of the trunk's own
  group as well.

The raft is still built to group 0's profile.

## Consequences

Bracing switched on in a group braces that group, whichever it is. A plate saved with
bracing on in its first group only loses the braces between trunks of its other groups
until it is switched on there too. Reopen this if the finer brace between a thick and a
thin trunk proves too weak to hold the thick one.

## Alternatives considered

### Keeping bracing the plate's, and saying so in the panel

One switch for everything is simple, but the Brace tab sits in a profile, and a profile
is what a group is built to: a number there that silently belongs to another group is the
confusion this was raised about.

### Tying two groups by the thicker brace

Holds the thick trunk better and swallows the thin one; a brace is already held to the
width of the trunks it meets (ADR 0242), so the thin trunk decides in either case.

### The option that won, and what it costs

Which group a brace between two belongs to is a rule the user cannot see, only infer from
its colour.

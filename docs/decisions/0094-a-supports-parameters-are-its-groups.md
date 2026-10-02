# 0094. A support's parameters are its group's

- **Status:** Accepted
- **Date:** 2026-09-26

## Context

One profile for the whole plate is not enough: a bust wants thick supports under the chin
and thin ones on the face, and a rim of small pillars around a thin skirt. One answer is
tags — a support carries a tag, a tag carries a preset — with a support's own numbers
editable and copied between supports. Another is a list of manual support types, chosen at
placement.

The pipeline so far took one `SupportProfile` for everything: the tip, the pillar, the
branching, the foot, the raft and the bracing all came out of it.

## Decision

A support carries the number of a group, not a profile of its own. `Profiles` is the table
of group profiles a plate is built with; `SupportPoint` carries `group`, and every tree
grown from it is stamped with the same number. Group 0 is what an out-of-range number
falls back to and what the raft and the bracing — which belong to the plate rather than to
any one support — are built from.

Growing runs once per group, so a trunk is only ever shared by supports of one shape: two
profiles merging into one strut would leave nothing to build that strut to.

The table lives in the window's tool rather than in the scene, beside the profile the
panel has always edited, and the scene stores only the number each support carries.

## Consequences

Retuning a group retunes every support in it, which is the point: a plate is a handful of
shapes, not a hundred loose numbers. Moving a support to another group is the edit that
replaces "delete it and click again with different settings"; carrying its parts about is
ADR 0095.

Because merging is per group, splitting a bed of supports into two groups costs resin: the
two halves no longer share trunks. Nothing warns about that.

A group's numbers are not on the undo stack, in the same way the single profile never was
— only which group a support belongs to is. The signal to reopen is a user losing a tuned
group to a stray click.

There is still no per-support diameter. A support that needs its own is given a group of
its own, which is one more entry in a list rather than one more field on a point.

## Alternatives considered

### A profile per support

A profile on every support, with copy and paste between them. Rejected on weight and on
merging: a `SupportProfile` is some hundreds of bytes against a `u16`, and two neighbours
with slightly different numbers could never share a trunk, so a hand-edited plate would
quietly stop branching.

### Groups stored in the scene, with their profiles

Would put group parameters on the undo stack and let a copied model carry its own tuning.
Rejected for now: it duplicates the same profile across every model on the plate and needs
a hash of a `SupportProfile` to keep the undo fingerprint honest.

### The decision above, and what it costs

Group numbers are positional, so dropping a group renumbers the ones above it and every
support has to be walked to keep up. And a group is plate-wide: two models cannot have a
"thin" that means two different things.

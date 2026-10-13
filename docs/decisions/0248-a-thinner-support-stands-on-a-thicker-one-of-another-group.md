# 0248. Stand a thinner support on the trunk of a thicker one of another group

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

Growing runs once per group, so a trunk is only ever shared by supports of one shape
(ADR 0094). Since a profile chosen from the list became a group of its own (ADR 0247), a
part routinely carries two or three kinds side by side, and each kind stood its own forest
of trunks and feet next to the other's: twice the resin, twice the pads to cut off, and
thin trunks left standing alone beside thick ones that could have carried them.

## Decision

Groups are grown thickest first, by the width of their pillars, and what a group has stood
— every strut and every trunk, the head under a tip apart — is a **carrier** for the
groups after it (`Carriers` in `branch.rs`). While a thinner group merges, a front may run
into a carrier as well as into another front:

- only into one at least 0.05 mm wider than itself, so what carries is what was built to;
- by one strut, leaning no further than its own profile lets a branch lean and reaching no
  further across than half its own merge distance, clear of the model: two fronts a merge
  distance apart meet in the middle, so that is as long as a strut ever is, and one twice
  as long across a cavity is a lever;
- at the highest point of the carrier it reaches, found by halving, and on a trunk no
  lower than the top of what the trunk stands on.

Both kinds of meeting sit in the one heap, highest first, so a tip beside a thick branch
joins it before it would merge with a far sibling, and a merged front is offered the
carriers again. A front that ran into one is a tree of its own with no trunk and no foot:
its `Landing` is the meeting point with `on_support` set beside `on_model`, since
everything that asks whether a support needs a foot, a raft pad or a brace already asks
`on_model`. The mesher ends its last strut plain on the carrier's axis, with neither the
upside-down tip of a landing on the model nor a small pillar's width. `on_support` is
written with the landing of every stored tree and reads as false from an older file, so
the manifest stays version 4.

Tips still merge within a group only: a joint of two shapes has no profile to be built to.
Every grown tree is grown again on each rebuild, so the order the groups were placed in
does not matter: thin supports placed first lean onto thick ones placed after them.

## Consequences

Two kinds of support on one part share branches, trunks and feet. Struts of two groups
can still cross where neither is in the other's reach: nothing keeps one support clear of
another. A tree frozen by hand keeps its
landing where it was, so carrying the thicker trunk away leaves the thinner one ending in
the air until it is moved or grown again; the trunk carrying it is not sized for the tips
leant onto it. Reopen this if either is seen on a print.

## Alternatives considered

### Merging tips of different groups in one pass

The fewest struts, but the strut below a mixed joint has two profiles and no rule for
which it is built to.

### Leaning whole trees onto trunks after every group is grown

Simple, and what was tried first: a thin tree that had already merged low and stood its own
trunk only met a thick trunk, never a thick branch beside its tips, and one that had landed
on the model was left there. The struts of the two kinds still crossed.

### Either tree onto the other

A thick tree leant onto a thin trunk puts the load on the part least able to carry it.

### The option that won, and what it costs

A tree's foot now depends on trees of other groups, so retuning one group can move where
another's supports stand.

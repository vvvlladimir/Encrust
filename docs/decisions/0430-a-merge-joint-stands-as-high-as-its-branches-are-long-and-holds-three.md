# 0430. Stand a merge's joint as high as its branches are long, and hang three branches at most

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

Merging (ADR 0040, 0338) took a joint anywhere over the plate. A pair refused for its struts
was dropped 1, 2, 4 … mm until it met, so under a crowded part tips met a millimetre over a
foot on 6 mm branches. That is a fan of levers on one point, and it saves nothing: printed
layer by layer, a merge saves the height of the one trunk under its joint. Each lean adds
length on top of that. A tip could also come down onto a neck or joint that already held
others, and `shortcut` (ADR 0428) moved branches onto the joint under. So one point could
hold six branches. On the crowded-balls bench, 19 of 198 joints stood lower over their landing
than their longest branch was long, and one held six branches.

## Decision

- **A joint stands at least as high as its longest branch is long.** In `offer` the height is
  taken over the plate. In `accepts` it is taken over the landing under the joint: the top of
  its foot on the plate, the face on the model. A pair dropped below that is not offered
  again, so the drop stops there. `shortcut` holds a branch it moves to the same rule over the
  plate.
- **A joint holds at most `MOST_BRANCHES` = 3 branches.** A front counts the branches at its
  node. One that is full is not met at its node, and `shortcut` moves no branch onto a full
  joint.

## Consequences

`cargo bench -p core-supports --bench branch` prints how steady the trees stand. On the
crowded balls, low joints fell from 19 of 198 to 0, and the most branches at a joint from 6
to 3. That came with 153 trees against 139 and 22 417 mm3 of support against 21 887, 2.4 %
more. On the lifted 40 mm ball the most branches at a joint fell from 6 to 3, and trunks
and volume stayed within one tree and 1 %. A pair too low to merge now stands on two
trunks. Reopen if prints show those trunks standing where a short joint held before, or if
volume on real models grows past a few per cent.

## Alternatives considered

### Keep a heavy trunk upright and lean the light branch onto it

Meant to keep a trunk under the middle of its load, and tried with the light side carrying
half or a third of the heavy side's tips. With half, the crowded balls stood on 171 trees and
24 515 mm3. With a third, 153 trees and 22 606 mm3. In neither did a landing move closer to
the middle of its tips, on average or at worst. A joint on the heavy column sits lower than
the symmetric one, so it fails the height rule more often and saves less.

### A fixed least height, say 3 mm

It does not scale with the lever a branch is. A 1 mm branch meeting 2 mm up is sound, and a
6 mm one meeting 4 mm up is not.

### The option that won, and what it costs

More trunks where tips are low or crowded, and a little more resin. The height in `offer` and
`shortcut` is only a bound over the plate. A joint over the model is held to its landing only
in `accepts`, and a branch `shortcut` moves is not.

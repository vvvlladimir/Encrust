# 0393. Warn of a cavity with no drain hole and of a solid model worth hollowing

- **Status:** Proposed
- **Date:** 2026-11-02

## Context

Two resin mistakes go to the printer unsaid. A model hollowed and given no drain hole
prints its cavity full of liquid resin: wasted, and in time it cracks the wall. The window's
drainage check finds it, but only when pressed in Drain; the command line runs that check on
every `--hollow` run and prints `trapped resin`. A large solid model costs resin and pulls
harder on the film, and nothing says that hollowing would save much of it. No published rule
says when to hollow; guides give a wall of about 2 mm and leave the rest to judgement.

## Decision

**`core_volume::holds_resin`** answers whether a cavity keeps its resin: grown inward, its
floor closed, and none of the model's holes of kind `Drain`. A walled or support hole drains
nothing. It reads settings and holes only, so the window asks it every frame.

**`core_volume::hollowing_saves_mm3`** estimates what a wall would save in a solid model:
its volume less its surface times the wall, under the model's placement. It names the saving
only when it is at least `MIN_SAVED_MM3` (10 ml) and `MIN_SAVED_SHARE` (40 %) of the model,
so a figurine, a sword or a thin plate stays quiet. The wall is the one Hollow would build
for that model.

Check lists both under **On the plate**, beside the other placement issues, as soon as the
models stand still: `hollow with no drain hole` and `worth hollowing`. A click picks the model
and opens the tool that deals with it, Drain or Hollow; the row carries no figure, as the
estimate is rough and Hollow states what it saved once run. The command line's
`inspect` prints the same saving at the default 2 mm wall; its drainage check already covers
the sealed cavity.

## Consequences

Both warnings are free: the first reads settings, the second is one parallel pass over the
faces, kept until a model, its shape, its holes or its wall change. The sealed-cavity warning
trusts that a drain hole reaches the cavity; one that misses it, or a pocket an infill seals
off, is found only by the full drainage check. Surface times wall overstates a convex wall, so
the saving is understated there and a borderline model may go unnamed. Reopen if users ask
for the thresholds to move, or if a hole that misses the cavity proves common.

## Alternatives considered

### Run the drainage check in Check on every change

It finds every pocket, a hole that misses included, but cuts every layer of every model again
each time the plate changes, beside the stack Check already reads.

### Warn by mean thickness instead of saved resin

Volume over surface reads how thick a model is regardless of its size, so it names small thick
parts that would save a millilitre: noise the user learns to ignore.

### The option that won, and what it costs

Two fixed thresholds stand in for judgement, with no source behind the numbers but common
sense, and the sealed check believes a drain hole drains.

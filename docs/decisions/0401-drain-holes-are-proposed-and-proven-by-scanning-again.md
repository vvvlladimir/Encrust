# 0401. Propose drain holes and prove each by scanning again

- **Status:** Proposed
- **Date:** 2026-11-02

## Context

A hollowed model drains only through a hole someone placed: a click in the window, or
`--drain-at X,Y,Z` picked by hand, which is the weakest step of a prepare a script drives. The window's "drill a hole
into each" goes from the nearest surface to the pocket's floor, which may be the lid of a
seated box and is never checked. The pieces exist apart: the drainage scan in `core-supports`
finds each pocket and its floor (ADR 0072), `core-volume` places and drills holes (ADR 0075,
0346), and slicing is `core-slicer`'s. `core-volume` and `core-slicer` are peers; only
`core-engine` reaches all three.

## Decision

`core_volume::holes_out` proposes the holes into one cavity, best first: rays cast out of a
point inside it — straight down, eight round at 45° below level and eight level, or the same
upward for a vent — each followed through at most 32 faces through the `Bvh`, the hole drilled
square into the face it leaves by, a diameter past the wall there, carried through the wall by
`pierce`, and half a millimetre short of the cavity's far side, cast for across it. A mouth off
an edge or a fin narrower than the hole, within a millimetre of the plate a model stands on, or
into a cavity too thin to stop in, is dropped; a drain ranks by its height less a radius for a
face looking down, a vent by its height alone.

`core_engine::suggest_drains` scans the model as it prints at the run's layer height, takes
each trapped pocket's proposals and proves them: a round drills one untried proposal a pocket,
clear by two diameters of every mouth already chosen, scans once, and keeps a hole whose pocket
no longer appears. After each round's scan every hole kept so far is checked again: one whose
pocket is found again was credited for a neighbour's failed try, and is dropped, its pocket
trying its next proposal. A pocket is still trapped while any box of a scan overlaps its own.
At most `MOST_TRIES` = 4 rounds for drains and 4 for vents, a vent proven with only the vents
drilled, then one scan with every hole for what is left: at most ten scans. A pocket nothing
drains is reported trapped. The holes are `HoleKind::Drain`. `trapped_in` is the scan, shared
with the window's drainage check.

The command line takes it as `--drain-at auto`, beside points, with a vent unless
`--no-drain-vent`; it runs after hollowing and before supports, in the pose the model prints
in, resting on the plate unless the lift its supports will give it, worked out first as for a
point (ADR 0400), stands it clear. `--json` adds `auto_drains` per model: `pockets`
(`slicing.trapped`'s fields and `drained`), `holes` (`pocket`, `role` `drain` or `vent`,
`at_mm`, `axis`, `diameter_mm`, `depth_mm`) and `scans`, in the plate frame of ADR 0400.

The window's Drain tool takes it as Auto, the companion of Check drainage at the tool's foot:
it is that check carried further, so it stands beside it rather than in a section. It searches
each picked model where it stands — its shell, its cuts, turned and lifted as the plate holds
it, resting on the plate where its lowest point is — on a worker through
`suggest_drains_heard`, which hears each scan and stops between windows, so the bar fills
over the most scans the search can take and Cancel drops what it found. Each hole comes back
into the model's own space, its sizes through the model's least scale as a click's are, and
is drilled as a click drills it; what is left trapped is marked as the check marks it. All
land in one frame, so one undo takes them back (ADR 0257); a model moved or remade while
the search ran gets nothing. It always vents, as the command line does by default: a switch
would be a field of the project's `DrainState` for a choice nobody has asked to make.

## Consequences

A run that drains reports `slicing.trapped` empty, and one that cannot stays unclean as
before, so an agent tries `auto` first and a point only where it fails. Each scan cuts the
whole model, so the search costs up to ten slices of it; the `drains` group of
`core-engine/benches/plate.rs` times it at 0.46 s, four scans, on a 40 mm ball at 0.05 mm layers. Supports placed after the holes land on the drilled surface but
not clear of a mouth's rim. The window cannot ask for a search without a vent; add a switch
if a user asks for one. Reopen if a pocket the search drains is reported trapped once written
— a scan at the run's layers that disagrees with the file's.

## Alternatives considered

### Proposals only, unproven

One scan, the nearest surface or the lowest mouth drilled blind, as the window's "drill a hole
into each" does. A hole that lands in the solid beside a cavity, or through a second cavity on
its way, is found only when the file reports it — the failure an agent cannot recover from.

### The search in `core-volume` or `core-supports`

Either would need the other and the slicer: `core-volume` may not reach `core-slicer`, and
`core-supports` has no business drilling. The geometry that needs no stack stays in
`core-volume` beside `hole_at`; only the loop that drills and scans is in `core-engine`.

### Scanning only the band a pocket spans

Cheaper per try, but a band's first layer reads as the plate and opens whatever crosses it,
so a proof would depend on where the band was cut.

### What proving costs

Up to ten slices of the model where one sufficed, and a pocket whose every proposal fails is
dearer still. Rays from one point miss a cavity reached only round a corner, and a lattice
that crosses every ray pushes its holes to the end of the ranking.

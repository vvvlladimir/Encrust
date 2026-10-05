# 0189. Resin leaves through a hole, and a lattice never closes a cell

- **Status:** Accepted
- **Date:** 2026-10-05

## Context

Hollowing had a third mode, `BottomThrough`: the cavity's cross-section carried down through
the underside of the model so the resin ran onto the plate. It only means anything for a
model standing on the plate — lifted or on supports it opens into air — and it is a second
field, a prism clipped against the model (ADR 0058, 0059, 0063), a variant in the window, in
`--hollow-mode`, in a plate file's `mode`, in a saved project, in the lab matrix and in two
documents. A plate hollowed that way still reported trapped resin, with nothing said about
why.

A hive or grid wall ran from the floor of the cavity to its ceiling, so every cell was a
closed box. They were not reported as dozens of pockets only because of a defect: `walls`
walks a footprint segment in chunks and dropped the chunk that left the cavity, so a wall
stopped up to a chunk short of the side it ran into and a corridor of air ran along the
cavity's wall, joining every outer cell into one pocket. Carrying the walls to the side —
which is what a lattice has to do to bond into the shell — would have sealed each cell on
its own.

What the user actually needs is for the resin to reach a hole, and holes and channels are
already the tool for putting one where it suits the part (ADR 0075, 0076).

## Decision

`HollowMode` is `Internal` and `External`. Nothing takes a floor out of a cavity: a hole or
a channel does that, anywhere on the model. A project written with the old mode does not
open, which is acceptable while the format is being rewritten.

A hive or grid wall leaves an opening `OPENING_MM` = 1.5 mm high over the floor of the
cavity and under its ceiling, everywhere but within `POST_REACH` = a quarter of a cell of a
junction. A post therefore stands at every junction, the open middle half of each cell side
is a bridge short enough to print, and the resin runs from cell to cell towards whatever hole
was drilled. The open wall is one walk along its whole segment, so it merges into as few
boxes as it did before (ADR 0060); the posts alone are stood stretch by stretch, as a cap at
the floor and one at the ceiling of each span.

A wall is carried to the side of the cavity it runs into, walked a lattice step at a time
past the last chunk that held (`meet`), and fused in by the same bond that already fused it
vertically.

The drainage check of ADR 0072 follows every hollow run and every cut: the window starts it
on the frame a run ends and whenever a hole or a channel goes in or comes out, marks each
pocket in its own red, and offers to drill into each from the Hollow panel. A change while a
check is running is kept rather than queued, so clicking hole after hole costs one check
after the one in flight.

## Consequences

One hole drains a lattice. A cavity filled with a hive and no hole reports one pocket
because it is one pocket, and the window says so without being asked, with the places marked
where the holes go.

The lattice carries less at the floor and the ceiling of the cavity. The opening is
measured from the surface the wall is welded to, so the middle half of a wall needs
`2 (OPENING_MM + half the shell's wall)` of cavity under it — 5 mm for a 2 mm shell — and
below that it is the posts at the junctions alone. A cavity that shallow leans on the shell,
which is what held it before a lattice was asked for.

The posts cost contours, which is what a dense lattice is priced in. On a 60 mm ball with
2 mm grid cells at 15 % and 0.05 mm layers, the busiest layer goes from 56 contours to 830
and slicing from 13 ms to 97 ms; at the 5 mm cells the window offers, from 24 to 206. A hive
pays almost nothing — 1226 to 1402 at 5 mm — because its edges are one cell long and were
never merged across cells. Both sit well inside the 37 139 contours a layer that ADR 0060
already calls real, and the open wall itself merges as it always did. Reopen if a grid's
posts ever have to be thinned to every other junction.

A hollow run and a placed cut now cost a drainage check as well, which cuts the plate again
at the layer height (ADR 0072) — on a plate big enough, that is a wait after every click.
Reopen if that check grows past the run itself, or if 1.5 mm proves too tight for a thick
resin to run through.

## Alternatives considered

### Keep the mode and open the cells with it

Leave `BottomThrough` and let it open the lattice at the floor as well. It drains only a
model standing on the plate, it cannot help a cell under the ceiling, and it keeps the second
field and the special case in five places for a case a hole already covers.

### An open pattern by default

Make Scaffold the default and leave Hive and Grid closed. Scaffold braces sideways but
neither hive nor grid is then usable on a tall part, and those two are the patterns that
stand on the plate and print cleanest.

### Mark every closed cell and let the user drill

Report each sealed cell and offer a hole into each. Dozens of pockets and dozens of holes
per model, which is work the lattice should not be creating.

### The option that won, and what it costs

Openings weaken the lattice between its posts, and the posts multiply the contours on the
layers they cross by about fifteen on a grid. Dropping the mode breaks a project saved with
it.

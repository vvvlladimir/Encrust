# 0240. Tie trunks along a Gabriel graph, and lean a lone trunk into reach

- **Status:** Accepted; how far a trunk leans and towards what is superseded by 0250
- **Date:** 2026-10-12

## Context

`braces` tied every pair of trunks within `max_spacing_mm` (ADR 0043, 0044). Three trunks
standing near each other got three crosses per level and a dense run got one for every
pair in reach, most of them crossing each other and holding nothing the shorter ones did
not. At the same time a long thin trunk further than that reach from any other got no
brace at all, and that is the one that sways on the peel.

## Decision

**Trunks are tied along the Gabriel graph of the trunks tall enough to brace, cut at
`max_spacing_mm`.** Two trunks are neighbours when no third stands inside the circle
whose diameter is the line between them. The graph is planar and contains every trunk's
nearest neighbour and the minimum spanning tree, so it is connected wherever the old rule
was and never crosses itself in plan.

**A tall trunk out of reach of every other leans towards its nearest neighbour**, on a
knee at the branch angle, until it stands three quarters of the reach away — only with
branching on, and only where the lean clears the model, lands on the plate and leaves a
trunk still tall enough to tie.

## Consequences

- Three trunks in a row carry two crosses a level, not three; a triangle keeps all three.
- The pair test reads every tall trunk for each pair in reach, cubic in the worst case;
  a grid is the answer when a plate of thousands shows it in a profile.
- A gathered trunk's foot is no longer under its tips, and a lean of many millimetres is
  resin spent on stiffness.
- Reopen if gathered trunks crowd one neighbour: gather towards the two nearest.

## Alternatives considered

### A cap on braces per trunk, nearest first

Simple, and it leaves the crossings: the nearest three of four trunks in a square are
its two sides and its diagonal.

### The relative neighbourhood graph or the spanning tree alone

Sparser, and a tree has no cycle to resist racking: a triangle of trunks would be tied
on two sides.

### Stretch the brace to the lone trunk instead of moving it

Nothing moves, and a brace three spans long is as thin a strut as the trunk it was meant
to steady.

### The Gabriel graph and the lean, and what they cost

Two long ties across a square of trunks are gone with the diagonal, so a square racks
more easily than it did.

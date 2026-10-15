# 0274. Score an orientation as it stands on supports, by its tips and by Stefan's pull

- **Status:** Accepted, the tip's cost superseded by 0275, Stefan's pull and its copy in `core-plate` by 0281
- **Date:** 2026-10-12

## Context

ADR 0087 scores a candidate as if it stood on the plate. The window and the command
line lift a supported model off the plate first, so its underside hangs like any
other surface, yet the score counted that underside as footprint and took a bonus off
for it. A tip lower than everything around it has almost no area, so it cost nothing,
although a support has to stand under it. A ceiling high over the plate cost the same as
one just above it. The peel term was the largest cross-section's area, but
`core-analysis` judges a cured layer by Stefan's pull, which goes with each piece's
torsion constant: a thin strip and two pieces apart pull far less than one disc of the
same area. And the overhang angle stayed at 45° whatever the support profile said.

`orient-lab` (`docs/cli.md`) measured what each goal actually prints on supports.
Shortest print stood slabs on their edge, at up to 25 times the layers of lying flat, and
Fewest supports left an island on a dome.

## Decision

`OrientSettings` carries `on_supports` and `contact_spacing_mm`, and its
`max_overhang_deg` is the support profile's own: the window and the command line fill
all three from the profile the model is supported with.

- On supports nothing rests on the plate: the underside is overhang, and the footprint
  bonus is gone.
- `Score::low_points` counts the vertices lower than every vertex they share an edge
  with, off the plate. Each costs the overhang of a flat patch one contact holds,
  `contact_spacing_mm` squared. Vertices are joined by position, so a triangle soup
  reads as its surface.
- `Score::support_mm3` is each overhang times how high it stands over the plate. It adds
  half again the overhang held for support standing a model's own length tall.
- The second pass measures each cut section's pull, `A⁴ / (4π² J)` per piece, the
  formula `core-analysis` uses, and scores the area of the disc pulling as hard. A disc
  pulls the most an area can, so the shadow stays an upper bound. `core-slicer` exposes
  `covered_pieces` for it, the pieces `covered_area` already found.
- Each goal states its own weights instead of multiplying the base ones. Height is a
  tie-break, at 0.05, for every goal but Shortest print, where it is 1.5.

## Consequences

Over `orient-lab`'s 16 bodies, against 0087's score:

- Shortest print takes 50–81 % fewer layers on the slabs, the coupon and the relief tile,
  but lays a peel slab flat at 144 N, past the 30 N `core-analysis` names.
- Fewest supports needs fewer contacts on the pyramid and the coupon, and
  leaves the dome without its island.
- Clean top keeps a cup open side up, so its floor seals a suction cup.

A goal that wins its own reading by printing a risk is the signal for priorities over
the weighted sum, with a peel cap: step 5 of the orientation plan. Cups are not seen
until the winner is checked on the cured stack.

The 100k-face ball orients in 32 ms against 22.6 ms (`cargo bench -p core-plate`). The
vertex graph is built once a search, and the tips cost one pass over it per candidate.

## Alternatives considered

### Keep scoring on the plate, and lift afterwards

The window and the command line always lift a supported model, so the plate case is the
one that does not happen. It survives as `on_supports: false`, for a model printed
straight on the plate.

### Count support contacts by placing them for every candidate

Exact, and what `orient-lab` does to judge. Rejected inside the search: placement slices
the model at every layer, which is seconds a candidate against milliseconds for the whole
search.

### Move Stefan's formula into `core-geometry` and share it

One formula, one home. Rejected for now because `core-analysis` reads it off pixel
moments and depends on `core-raster` only. Sharing it means a new edge in the graph for
one line, while each side keeps its own tests against Stefan's disc.

### Weights per goal, and what they cost

They are still set by eye, now with a lab to check them against, and a goal still cannot
say "this, but not at that price". The tests pin the slab, the column, the cube and the
pyramid, and `orient-lab` the rest.

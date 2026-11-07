# 0404. Keep thin parts solid by opening the model with a ball

- **Status:** Proposed
- **Date:** 2026-11-05

## Context

A recipe's hollow step names `keep_solid_under_mm` (ADR 0403): the wall stays solid
wherever the model is thinner than that, so a finger, a limb or a blade is not hollowed and
does not print as a skin that snaps. Until now only a user's clicks kept the wall solid, one
blocker at a time (ADR 0396); a recipe runs unattended and has no clicks. Hollowing builds
a narrow-band field two voxels wide around the cavity's surface, and stores only the tiles
that band crosses (rule 7 of `architecture.md`).

## Decision

`HollowSettings::solid_under_mm` takes the cavity only inside the model opened by a ball of
radius `r`, half the setting: the model eroded by `r`, then dilated by `r`. A part thinner
than `2r` vanishes from the eroded solid and so from the opened one. Since a band reaches
two voxels, not `r`, each half is a narrow band built around its own surface: the model's
at `iso = -r`, extracted, and the eroded solid's at `iso = +r`, on the cavity's lattice.
A shell's cavity field is intersected with it; a mould's void is the opened model.
`cavity_field` does it, so its cache keeps it with the field. The how is in
`docs/design/hollowing.md`.

## Consequences

Every hollow mode honours it, in the window, the command line and a recipe through one
setting. A hollow with it costs two more band builds and one extraction: 2.1 s against
0.93 s on the `opening` bench's 40 mm ball. It is exact to a voxel and needs no tuning, but
it also rounds the cavity's own corners by `r` past the wall, a fraction of a percent of a
cube's cavity. Reopen it if the opened solid's build dominates a real model's hollow.

## Alternatives considered

### Blockers fitted by sampling the surface

Balls fitted under sample points as a click fits one (`fitted_blocker`), kept where the part
is thinner than the setting. Their count grows with the surface, and a thin part between
two samples is hollowed anyway.

### A thickness ray from each face

The width across under each face, as `across_mm` measures it, marking thin faces. Noisy at
edges and corners, where the ray grazes, and a mark on faces still has to become a solid.

### Opening on the field, and what it costs

Two band builds over surfaces the size of the model's, so a hollow with it takes about twice
as long, and the opened solid is held beside the field in the cache while kept.

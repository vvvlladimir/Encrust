# 0044. Cover a joint with a ball, bevel every foot, cross the braces

- **Status:** Accepted
- **Date:** 2026-09-19

## Context

Steps 7c to 7e were checked against `diagnose` and against the rasteriser, and both were
satisfied: every piece closed, every overlap unioned, every layer solid. Looking at the
result in the viewport showed four things none of those tests could see.

A branch **poked out through the far side of its trunk**. `docs/decisions/0041` extended
each strut past the joint it ended at by the parent's radius, on the reasoning that a
strut leaning at most 45 degrees moves sideways by less than the trunk's radius. That
reasoning left out the strut's own width: a strut of radius `r` extended by `R` along a
direction `θ` from vertical reaches `R·sinθ + r` from the trunk's axis, which is outside a
trunk of radius `R` as soon as `r > R(1 - sinθ)`. At the shipped numbers it always is.

A **trunk showed through the sole of its foot**. The trunk was run all the way to `z = 0`
so that it would overlap the foot, so the pillar's own circle was part of the footprint.

**Every foot met the plate on its widest edge.** A pad whose rim is the widest part of it
is the pad a blade cannot get under. Every MSLA slicer bevels that rim, and the skate of
`docs/decisions/0043` was worse than that: it was cocked at the toe, so it stood on an
edge and leaned.

**Cross braces were rungs, not crosses.** Horizontal struts at one height tie two trunks
together but do nothing about the pair leaning as one.

## Decision

**A joint is covered by a ball.** A strut now stops dead at the node it ends at, with no
extension at all. Every node that a strut hangs off gets a ball centred on it, of the
trunk's radius times `JOINT_SWELL`, drawn with `JOINT_RINGS` rings — an even count, so
one of them lands on the equator where the ball has to be at its full width. The swell is
there for two reasons: a strut ending at the centre of a ball of its own radius has its
end cap exactly on the surface, and a polygon's flats fall inside the circle they stand
for.

**A trunk stops inside its foot**, `FOOT_BITE` of the foot's height below the top of it,
rather than at the plate. Far enough to overlap, not far enough to show.

**Every foot is bevelled** where it meets the plate. A round foot gains a third ring:
full rim until `FOOT_BEVEL` of its height from the bottom, then in to `rim - bevel` at the
plate. A skate lies **flat** on the plate, with its footprint inset by `SKATE_BEVEL` from
its own top — the cock is gone. A tapering foot's top ring is never narrower than the
trunk standing in it.

The raft's walls lean the same way: the slab meets the plate on a rim pulled *in* by
`overhang_mm`, where `docs/decisions/0043` pushed it out. That ADR's reasoning — that a
wider base grips harder — is true and is the wrong trade: the raft is the piece the whole
plate is levered off by.

**A brace is a cross.** Each step ties the foot of one trunk to the head of the other and
back again, so the pair is braced against leaning either way, and the lattice looks like
the one every slicer draws.

## Consequences

Branches, trunks and braces all end where they say they end. The joint's ball is the only
thing that hides a seam, and it hides every seam at that node whatever the angles are,
which the old extension could not do for more than two struts.

A joint costs one ball: `facets × (JOINT_RINGS - 1)` quads, 60 quads at the shipped
twelve facets. On the benchmark's lifted ball — 37 tips on 5 trunks — meshing went from
25 µs to 39 µs and the support mesh from 943 mm3 to 918 mm3, because dropping the
extensions gave back more than the balls cost. The saving against straight columns is
unchanged at 56%.

`mesh_trees`'s volume test now states the bevel and the bite in its closed form, so the
numbers in it are checkable by hand rather than fitted.

The bevel is a fixed fraction of the foot's height, not a profile field. A user who wants
a sharper rim has no way to ask for one. That is deliberate for now — the parameter list
is already long — and the signal to add it is someone reporting that a foot will not come
off.

A ball at every joint is a sphere where an organic generator would put a fillet. It is rounder
than the old flat crossing and it is not smooth: the branch still meets it at an angle.
Reopen the look, not this decision, if that shows.

## Alternatives considered

### Mitre the strut against the trunk it enters

Cuts the strut's end cap to the trunk's surface, so nothing pokes out and nothing is
added. Rejected because the cut is against a cylinder, not a plane, so the end ring stops
being planar and the tube stops being a sweep — and at a node where three struts meet,
each has to be cut against the other two as well.

### Clamp the extension so the strut stays inside

One line: allow at most `(R - r) / sinθ` of extension. Rejected because it goes to zero
exactly when the strut is as thick as the trunk, which is the common case at the first
merge, and a zero extension is the pinched seam the extension existed to avoid.

### A ball at every joint, which is what we do

The honest costs: 60 more quads per joint, a visibly spherical bulge where a real slicer
would fillet, and a swell factor that is a constant rather than something derived. The
bevel and the bite are constants too. All four are numbers chosen to look right at the
shipped profiles rather than measured against anything, and a profile far from those may
want different ones.

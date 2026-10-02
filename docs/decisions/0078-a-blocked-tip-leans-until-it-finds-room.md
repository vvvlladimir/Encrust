# 0078. Let a blocked tip lean until it finds room

- **Status:** Accepted
- **Date:** 2026-09-23

## Context

With ADR 0077 checking the whole body, a straight column is refused wherever the model is
in its way: under a steep flank, in a groove, over a part that rests on the plate. On a
ball hanging over a second ball, that refused 39 of 70 tips — every one of them an
overhang that still needs holding.

`columns` threw a tip away the moment its own column had nowhere to stand, before `grow`
had seen it, so a tip could not be rescued by anything the forest knew. Yet the forest
already builds leaning struts: a merge is two of them.

The established answer searches a cone of bridge directions from the head for one whose
bridge and pillar both reach the ground clear of the mesh.

## Decision

`Column::landing` becomes an `Option`: `columns` resolves what it can and keeps the rest,
and where a tip stands is settled in `grow`, after merging.

A front the model leaves no room under bends a **knee**: the support leans to one side,
and the trunk drops from there. It happens between the two merge passes, so a bent front
is a front again and neighbouring knees share a trunk rather than standing as a palisade;
what is still blocked at harvest bends there instead. A merge is accepted when its
meeting point can stand *or* can bend, for the same reason. The knee is searched for over
eight directions and three reaches, shortest reach first so a support leans no further
than it must, at the profile's `branching.max_angle_deg` and no further than
`branching.max_merge_distance_mm`. A lean costs height, so the reach is also capped by
what is left under the front — a tip low over the plate can barely step aside. Both the
leaning strut and the trunk under the knee go through ADR 0077's beam.

A tip that neither merges nor finds a knee is dropped, and the panel's
standing-against-placed count is where that shows.

## Consequences

The 39 lost tips come back as 38: 69 of 70 stand, and none of them touches the model.
Supports around a steep flank now lean outwards, which is what they look like in every
other MSLA slicer. Bending before the second merge pass is what keeps them a forest: on a
80 mm ball lifted clear of the plate it is 46 trees over 155 tips rather than 56.

Bending costs up to 24 candidates for each blocked front, each a beam and a landing, and
it buys a second merge pass, so a plate whose supports are mostly blocked pays for a
search that mostly fails: `grow` on the benchmark ball goes from 446 µs to 830 µs, and
gives back 13 trunks and 41% of the resin against 16 and 33%.

The knee is the one place a support leans without another tip to meet, which makes it the
obvious hook for the painted and projected supports of step 13. It is also the crude part
of this: eight directions is a coarse cone, and the first reach that works wins rather
than the best one. A model that needs a 30-degree bearing gets a 45-degree one.

## Alternatives considered

### Keep dropping the tip

Honest, and what ADR 0077 shipped with. It leaves a model visibly short of supports
exactly where the geometry is hardest, which is the failure the step exists to end.

### Bridge onto the nearest standing trunk

The other established route, and cheaper to search: one candidate per neighbour rather
than a cone. It needs the trunks to exist before the blocked tips are resolved, so
harvest would have to run in two passes, and it cannot help a tip with no neighbour.
Worth revisiting when support groups arrive in step 13.

### The option that won, and what it costs

A knee is a joint in mid-air carrying a single tip, so it is weaker than a merge and adds
a strut that no other tip shares. Resin goes up for exactly the supports that were the
most awkward anyway, and the lean is chosen by a search whose first hit wins, not by what
would print best.

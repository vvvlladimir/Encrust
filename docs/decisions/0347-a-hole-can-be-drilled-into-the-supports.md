# 0347. A hole can be drilled into the supports

- **Status:** Accepted
- **Date:** 2026-10-25

## Context

A hole drilled into a model already cut the supports and raft its body reached when the
plate was baked (ADR 0322), but the window drew them whole, and nothing could be drilled
into a support or the raft on its own: picking met models only, and only their surface as
imported, so a click into a hole already drilled landed on the mouth it took and was
refused as a click on that hole. A raft under an open floor, or a sealed trunk, needs a
vent of its own; a stepped hole needs a narrower one drilled on from the floor of the first.

## Decision

A hole in the supports is a `DrainHole` of `HoleKind::Support`, kept with the model the
supports stand under, in the model's own space like its other holes. Supports are anchored
in that space (ADR 0028), so a tree or raft built again under it is drilled again, and the
hole is saved in the project with the rest. It takes no plug, `pierce` never deepens it and
`sleeves` puts no wall round it; its body cuts whatever of the model and its supports it
reaches, as every hole does.

The window keeps each model's supports as they print, `PrintedSupports`: every group its
cuts reach drilled through `drilled_reached`, and a `Bvh` a group, built on a worker
whenever the groups, cuts or placement change. They are what is drawn, and with the model's
printed surface (`support_surface`, drilled and hollowed) what the Drain tool's click, its
preview and its alt-click meet: `pick_printed` answers the model or the support group
nearest the camera. A click adds nothing only where the hole would lie wholly inside one
already drilled at least as wide, so a narrower or deeper hole drilled on from a floor is
drilled. The CLI drills `--drain-support-at` into the supports once they stand, its bodies
appended to the model's cuts already lifted with it.

## Consequences

The Drain tool drills anything that prints under the cursor, and the viewport shows every
hole open through what it cuts. Each support change costs a worker run that indexes every
group again, cut or not; a group the cuts miss keeps its mesh and normals. A hole into the
supports has no plug and never empties a cavity unless it reaches one. A tree whose piece
the engine refuses is kept whole, so its foot can stand on in a raft's hole.

## Alternatives considered

### Keep the hole with the supports

It belongs to the trees it was drilled in, but every rebuild — a model moved, a setting
changed — drops it, and it needs a drilling path and a file format of its own.

### Pick support meshes without a hierarchy

No worker run, but a ray tested against every face of a raft each frame the cursor moves.

### The option that won, and what it costs

A hierarchy a support group kept in memory, rebuilt on every support change, and a third
kind of hole every place that reads holes has to tell apart.

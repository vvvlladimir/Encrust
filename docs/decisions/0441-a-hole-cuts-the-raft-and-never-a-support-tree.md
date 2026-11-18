# 0441. A hole cuts the raft and never a support tree

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

A hole's body reached past the surface it pierced (ADR 0071) and cut whatever of the model
and its supports it met (ADR 0322, 0347). A drain drilled into a model's underside therefore
cut the tips and trunks of the trees standing in it: the very columns holding the model up
near its opening printed short or hollow. The Drain tool also drilled a tree on a click. A
raft is different: it lies flat on the plate, a hole through it only lets resin out, and a
raft under an open floor needs one.

The raft was meshed into the first group's column mesh, so nothing downstream could tell it
from a tree.

## Decision

A hole cuts the model and its raft, never a support tree.

- `mesh_groups` answers `GroupMeshes`: the column mesh of each group and the raft apart.
  `ModelSupports` keeps the raft beside the groups (`raft`, `drawn_raft`); `every_mesh` and
  `drawn_every` give both to what weighs, places or bounds the supports.
- `core_engine::Model` carries `raft` beside `supports`; `bake` drills only the raft, and
  `Stood` hands the raft apart for the CLI to carry the same way.
- The window's `PrintedSupports` drills and indexes only the raft. `pick_printed` meets a model
  or its raft (`Printed::Raft`); a ray through a tree passes it, so the Drain tool cannot drill
  one. `--drain-support-at` drills the raft nearest its point and is refused without a raft.

A hole of `HoleKind::Support` stays as it is saved; one drilled into a tree before now cuts
only the raft it reaches.

## Consequences

Trees under an opening print whole and keep holding the model at its edge. The raft is drawn
in the first group's colour and hidden with it, as before. A model the engine cannot read
still takes its cuts by the winding rule (ADR 0322), which subtracts from every body they
overlap, trees included; that fallback is unchanged. Reopen this if a sealed trunk is ever
needed: it would want a hole kind that names the tree it was drilled into.

## Alternatives considered

### Keep the raft in the first group's mesh and note its faces

Fewer call sites change, but every reader of the groups has to know that one range of one
mesh is not a group, and a drilled group would have to be cut apart and joined again.

### Tell the raft from a tree by its height

No new field, but a tree's foot stands in the raft's top and a skate's lip rises above it;
any height chosen cuts one or misses the other.

### The option that won, and what it costs

A field more on `Model`, `ModelSupports` and `Stood`, and every place that wants all of the
supports must ask for the raft too or miss it in a bound or a volume.

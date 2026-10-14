# 0266. Stand a support on a raft by its flare, without a pad

- **Status:** Accepted
- **Date:** 2026-10-14

## Context

A raft was a slab beside the supports: every support kept its pad on the plate and passed
through the slab (ADR 0043). The raft drew its outline around every foot at the pad radius
of the profile it was built from (ADR 0241, 0253), while each foot was meshed to its own
group's profile. A group with a wider pad than the first stood its pads out past the
raft's edge, and a pad as thick as or thicker than the slab stood up out of it, so what
should be one flat sheet under the part came off the plate as a sheet with lumps and
fringes. A pad exists to spread a single support's grip on the plate; on a raft the slab
already does that.

## Decision

**With the raft on, a support standing on the plate has no pad.** Its trunk ends on the
raft's top, `raft.thickness_mm` up, and widens there through the flare to the profile's
`lower_diameter_mm`, no longer held inside a pad's rim; the flare bites half the slab's
thickness into it. A skate foot is dropped too. `SupportProfile::footing_height_mm` is
the height a plate support starts at: the raft's thickness with the raft on, the pad's
otherwise, and every place that measured the room above a pad measures from it.

The raft's outline, a grid raft's pad under each foot and a connected raft's pads and
strips take each foot at the radius it meets the ground with, from its own group's
profile: the flare's bottom on a raft, the pad's rim without one.

Every group stands on the one raft, so the table a model is built with carries that raft
in every profile, not only the first (`core_engine::project::own_table`): with none given,
the first group's is copied to the rest.

## Consequences

- A raft is one sheet with flares on it; nothing of a support pokes out past its edge or
  above its top except the flare and trunk.
- A plate support on a raft is up to a pad's thickness taller, which the lift already pays
  for: the lift adds the raft's thickness (ADR 0238).
- The Shape panel's Raft drawing shows the flare on the slab with no pad when the raft is
  on; the Base drawing still shows the pad the raft replaces.
- Reopen if a flare alone proves too weak a joint to the slab on prints, for example by
  giving the flare a wider bottom on a raft.

## Alternatives considered

### Keep the pads and draw the raft around each foot's own pad

Fixes the overhang, but leaves a pad standing up out of a thinner slab and spends resin on
a grip the slab already gives.

### Build the pads into the slab, sunk to its thickness

The pad's top would sit flush, but a pad narrower than its bevel and a slab thinner than
the pad leave nothing for it to be; one rule for every pad and raft is the flare alone.

### The option that won, and what it costs

A profile's pad settings do nothing while the raft is on, and nothing on the panel says so
yet. A group's own `[raft]` is overwritten in the table it is built with, so a caller
handing the core a table by hand must keep the rafts equal itself, or a group whose raft
is off grows pads on the slab of another.

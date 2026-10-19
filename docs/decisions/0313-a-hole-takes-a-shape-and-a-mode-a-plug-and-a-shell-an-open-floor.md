# 0313. A hole takes a shape, a mode and a plug, and a shell can open its floor

- **Status:** Accepted
- **Date:** 2026-10-19

## Context

A drain hole was always round and always carried through the wall once the model was
hollow (`pierce`, ADR 0075), so it could not be a socket for a magnet or a pin, nor a
polygon whose pointed roof prints without sagging in a side wall. Nothing closed a hole
again after printing, and a model standing on the plate could only drain through holes.
Users of resin slicers expect all three: a hole mode, polygonal holes, a plug kept to close
the hole, and a shell open through its floor. `DrainHole` and the Hollow and Drain states
are written into project files and presets, so any change has to read the files already
written.

## Decision

`DrainHole` and `HoleSize` carry a `HoleShape` — round, or a polygon of 3 to 12 sides with
its corners on the diameter — a turn in degrees from the level, and a `HoleMode`: `Through`
is carried through the wall as before, `Blind` keeps its depth. All three are `serde`
defaults, so an older file reads as round, unturned and through. `HoleSize` moves into
`drain.rs` beside the hole it sizes.

`plugs` in `core-volume` makes the plug a hole took out of the wall as its own closed mesh:
the hole's outline moved in by a gap a side, its inner end flat at the wall where the model
is hollow and the hole goes through, at the hole's depth otherwise, its outer face sampled on
rings over the mouth and dropped onto the model's surface by rays cast out of the wall, as
`lift_for` reads a rim. `stand_on_end` turns it onto its flat end. The window lays one per
hole beside its model on a press of a button; the CLI does it with `--drain-plugs`. The gap
is 0.15 mm a side by default, and the fit is what prints: where a resin's tolerance
compensation is off, `Compensation::fit_drift_mm` — its hole and outer offsets — comes off
the gap.

`HollowSettings::open_toward` asks a shell to open through the floor facing that way, in the
space the wall is measured in; `plate_down` gives it for a placement. `carve` sweeps the
cavity's outline, read a band in from the floor, out past the floor before the blockers are
taken out of it. The window computes the direction from the model's turn, so turning an
open shell makes it stale; the CLI's model already stands on the plate, so it is straight
down. `HollowState::open_floor` and `Cavity::open_toward` are defaults in a file.

## Consequences

A hole can be a socket, and a polygon in a side wall needs no support. A plug closes flush
on a curved surface, costs a few hundred rays, and needs no closed mesh and no boolean, so a
torn scan gets one too. A model on the plate drains with no hole. The window still draws an
open shell's floor, since the model's own surface is kept and only outweighed; reopen this
if users read it as closed. A floor that is not level opens only where the cavity's outline
reaches it, and a cavity opened onto the plate is sealed by it until the print comes off.

## Alternatives considered

### A plug cut by the boolean engine

Intersecting the model with the hole's body through `core-csg` gives the exact material.
It refuses a mesh that is not closed, and on a hole in a side wall it sends half a dense
model through the engine, seconds rather than a frame. A sampled outer face is within a
layer of the surface on any face a drain is put in.

### An open floor as a third `HollowMode`

It reads as one choice in a picker, but every match on the mode would need it, and it is the
inside shell with one more cut, not another way to measure a wall.

### The option that won, and what it costs

Three more fields on a hole and one on a shell in every file, a plug that follows the
surface only at its sample points, and an open floor the model view does not show.

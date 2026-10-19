# 0313. A hole takes a shape and a plug, and a shell can open its floor

- **Status:** Accepted
- **Date:** 2026-10-19

## Context

A drain hole was always round, so it could not be a polygon whose pointed roof prints
without sagging in a side wall. Nothing closed a hole again after printing, and a model
standing on the plate could only drain through holes. Users of resin slicers expect all
three: polygonal holes, a plug kept to close the hole, and a shell open through its floor.
`DrainHole` and the Hollow and Drain states are written into project files and presets, so
any change has to read the files already written.

## Decision

`DrainHole` and `HoleSize` carry a `HoleShape` — round, or a polygon of 3 to 12 sides with
its corners on the diameter — and a turn in degrees from the plate's level, carried into
the model's own space as it is placed, both `serde` defaults, so an older file reads as round
and unturned. `HoleSize` moves into `drain.rs` beside the
hole it sizes. A `Channel` takes the same two fields and is dug as one prism of its outline, mitred at every
bend and carried round it by the least rotation, in place of tubes joined by balls, which
broke a polygon's outline at each bend. Every hole is
still carried through the wall once the model is hollow. The viewport cuts a polygon to its
own outline: each drain cut hands the shader its first corner and its count of sides.

`plugs` in `core-volume` makes the plug a hole took out of the wall as its own closed mesh:
the hole's outline moved in by a gap a side, its inner end flat at the wall where the model
is hollow, at the hole's depth otherwise, its outer face sampled on rings over the mouth and
dropped onto the model's surface by rays cast out of the wall, as `lift_for` reads a rim.
It is cut for the hole as it is cut, through the wall, and `laid_beside` stands it on its
flat end in a row past its model for both front ends. The window lays one per hole as soon
as the hole is cut, and again whenever the holes, the wall or the gap change; a plug knows
its model by `ObjectState::plug_of`, the model's place in the file, and a project opened
again keeps the plugs it was saved with. The CLI lays them with `--drain-plugs` and stops
where one would land off the plate or on another model. The gap is 0.15 mm a side by default, and the fit is what prints: where a
resin's tolerance compensation is off, `Compensation::fit_drift_mm` — its hole and outer
offsets — comes off the gap.

`HollowSettings::open_toward` asks a shell to open through the floor facing that way, in the
space the wall is measured in; `plate_down` gives it for a placement. `carve` sweeps the
cavity's outline, read a band in from the floor, out past the floor before the blockers are
taken out of it, then takes the cavity's mesh out of the model exactly, so the shell is one
closed surface with no floor where the cavity reached it: the cavity is cut across above the
opening and only its side toward the plate goes through `combine_exact_toward`, the rest
welded on, and where that cannot be done the whole model goes through `combine_exact`.
Where the engine refuses the model, the cavity is appended whole and outweighs the floor as
before. The window offers
the open floor as a third Shell choice, Bottom through, beside Inside and Outside; it is
the inside shell with `HollowState::open_floor` set, which a file without it reads as off.
The window computes the direction from the model's turn, so turning an open shell makes it
stale; the CLI's model already stands on the plate, so it is straight down.

## Consequences

A polygon in a side wall needs no support. A plug closes flush on a curved surface, costs a
few hundred rays, and needs no closed mesh and no boolean, so a torn scan gets one too. A
model on the plate drains with no hole, and the model view shows its floor gone. A model of
one shell pays for its bottom, 0.29 s on a half-million-face ball against 1.06 s whole; one
of several shells crossing at the floor goes through whole, about four seconds on a million
faces; a torn one keeps a floor the view draws and the slices do not. A floor that is not
level opens only where the cavity's outline reaches it, and a cavity opened onto the plate
is sealed by it until the print comes off. Plugs are models of their own: moved, deleted or
hollowed like any other, and laid again in a row beside their model when its holes change.

## Alternatives considered

### A hole that stops at its depth, a socket for a magnet or a pin

It was built as a `HoleMode`, through or blind. In use a blind hole and a drain looked and
printed alike for every hole a user drills to drain, and the choice only cost a row in the
panel and a field in every file, so it was taken out before anything built on it.

### A plug cut by the boolean engine

Intersecting the model with the hole's body through `core-csg` gives the exact material.
It refuses a mesh that is not closed, and on a hole in a side wall it sends half a dense
model through the engine, seconds rather than a frame. A sampled outer face is within a
layer of the surface on any face a drain is put in.

### An open floor as a third `HollowMode`

It reads as one choice in a picker, and the picker shows it so, but every match on the mode
in the core would need it, and it is the inside shell with one more cut, not another way to
measure a wall.

### The option that won, and what it costs

Two more fields on a hole and a channel, one on a shell and one on a model in every file,
a few planes tried through the cavity before the whole model goes through, a plug that
follows the surface only at its sample points, plugs laid again from scratch when their
model's holes change, and a floor opening that falls back to a drawn floor on a torn model.

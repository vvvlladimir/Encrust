# 0113. Write only the grey the panel will cure

- **Status:** Superseded by 0139
- **Date:** 2026-09-27

## Context

`Shading::Coverage` gives a pixel the grey matching the exact area the layer covers
(ADR 0039), which is more accurate than any level count another slicer offers. It is also
more grey than the hardware holds. A pixel is not a dimmer switch: below roughly half the
8-bit range the resin under it stops curing rather than curing less: published measurements
report imperfections from brightness 190 down and a complete void by 160. A mask that
asks for 26 therefore does not print a thin edge — it leaves liquid resin behind a wall
that looks finished, which is the failure this is here to prevent.

Two other slicers' controls sit on the same fact. An "anti-aliasing level" of `n` is a
ladder of `n` greys, and at level 8 every value is back to black or white, which is why the
advice is to stay at 2 to 4. A written file records that level, and ours recorded a
constant.

## Decision

`core_raster::Grey` carries the two limits and is applied after coverage:

- `levels: Option<NonZeroU8>` rounds to a ladder of `256/n − 1, …, 255`, matching what a
  file written at anti-aliasing level `n` elsewhere contains. `None` keeps all 255.
- `floor: u8` is the dimmest grey to write; anything below it is written black.

The floor is `Display::grey_floor` on the printer profile, because it is a property of the
panel and the light behind it rather than a taste, and it defaults to **128** — a profile
that never mentions it gets the floor, since the physics does not depend on whether a TOML
file was updated. `--grey-floor` overrides it for a machine that has been measured;
`--grey-levels` and the Slicing panel ask for the ladder. The `.goo` header's grey-level
field now carries the count instead of a constant `1`.

## Consequences

An edge comes out a fraction of a pixel thinner than exact coverage asked for, and the
accuracy `analytic_areas` measures drops by that much. In exchange no mask carries a grey
that would not hold, which is the difference between a thin wall and a sealed pocket of
resin.

Every existing user's output changes on upgrade: a profile with no `grey_floor` now floors
at 128 rather than keeping every grey. That is the point, and it is the reason the value is
a profile field rather than a constant — a panel measured to do better is one line of TOML
away.

The floor is a panel field but the threshold is really the panel *and* the resin: a slow
resin needs a higher floor than a fast one at the same exposure. The signal to reopen this
is a resin whose thin edges fail at a floor another resin prints, and the answer is then a
term on `MaterialProfile` combined with this one, not a replacement for it.

## Alternatives considered

### Snap the dim pixels to white instead of black

Keeps the wall solid and never leaves liquid. It lost because it grows the model by up to a
pixel on every edge of every layer, which is a dimensional error a user cannot correct,
where losing a fraction of a pixel is within what the panel's pitch already costs.

### Leave it to a post-processing pass that heals the file afterwards

Honest, and it keeps the rasteriser pure. It lost because the pass would have to expand the
runs to pixels and write them back, on a stack whose whole representation exists to avoid
that, and because a slicer that writes a file it knows will not print is not much of a
slicer.

### Match the shipped grey ladder and drop exact coverage

The straightforward reading of what step 16 asked for. It lost on evidence: coverage by
exact area is an order of magnitude more accurate than binary in our own test, and a ladder
of 8 is strictly coarser. The ladder is therefore an option over coverage, not a
replacement for it.

### The option that won, and what it costs

A default of 128 is a guess dressed as a measurement. It comes from community testing on
other machines, not from a panel on this desk, and the shipped profiles are already marked
`UNVERIFIED` for exactly this kind of number. A machine that holds grey down to 64 prints
slightly small until someone edits its profile, and nothing in the application tells them
to.

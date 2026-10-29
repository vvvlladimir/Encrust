# 0371. A plate inset warns and never clips

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

The maker's settings for a machine can give a "build area offset": four margins in mm,
left, right, top and bottom, meant as boundary lines on the plate. It is on for
three machines we ship: the Elegoo Mars 2 and Mars 2 Pro (1.47, 1.15, 0.26, 1.3) and the
Mars Pro (0.26, 1.3, 1.15, 2.5). The QIDI 6.08 turns it on with zero margins, and the Mars 3
and Mars 3 Pro carry the numbers but leave it off.

None of the four margins appears in the header or settings of a known file for
the Mars 2 Pro; 1.15 turns up only at unaligned offsets inside the image data. The offset
does not reach the file: it guides the person placing models.

## Decision

`PrinterProfile.plate_inset`, `[plate_inset]` in TOML, holds the four margins in plate axes:
`min_x_mm`, `max_x_mm`, `min_y_mm` and `max_y_mm`. We read the maker's left, right, bottom
and top as −X, +X, −Y and +Y, because plate axes are the panel's. `PlateInset::area_mm` is
the one place the usable rectangle is computed.

The inset is a soft boundary:

- the viewport dashes it on the plate, draws the plate darker inside the band, and hatches
  whatever stands in it, as it hatches what stands past the build volume;
- the printer form offers it on every machine: a switch, then the four edges. The three
  machines above ship with it on, the rest with it off;
- `encrust inspect` and `batch` report how far a model reaches into it (`inset_mm` in the JSON),
  and the model still fits;
- slicing, arranging and placing ignore it: a model in the band slices whole.

## Consequences

- A user sees where the maker would rather not have a model and can choose to use it anyway.
- Arrange can still put a model in the band; the hatching shows where it did.
- If the left/right/top/bottom reading is wrong, a dashed line sits about a millimetre off on
  one side. A machine, or a file showing the lines, would settle it.

## Alternatives considered

### Keep models out: arrange, place and the fit check work inside the inset

Considered and turned down: the plate is usable to its edge, and the user wants to decide.
It would also have moved `core-plate` from a plate size to a rectangle at every caller.

### Clip the masks to the inset

Throws away what the panel can print, which the maker does not ask either.

### The option that won, and what it costs

One more uniform corner in the shader, a darker band and a dashed loop on the plate, a
switch in the printer form and one optional profile table. The inset is only advice, so a model in the band prints as sliced.

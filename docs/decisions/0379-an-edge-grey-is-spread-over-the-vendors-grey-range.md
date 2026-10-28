# 0379. Spread an edge grey over the vendor's grey range

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

ADR 0113 and 0139 gave the mask a grey floor: an edge grey below it is written black, and the
floor defaults to 0. The vendor's slicer has a Gray Range instead, a dimmest and a brightest
edge grey per machine, and 19 of the machines we ship default to one narrower than 0..255
(min 63, 95 or 127; max 204 on two). Three vendor files of one model at 0..255, 127..255 and
0..200, compared pixel by pixel over all 648 layers, show what it does:

- an edge grey `v`, neither black nor full white, is written `min + max(v, min) · (max − min) / 255`;
- black stays black and full white stays 255, whatever the range;
- so at 127..255 every edge grey is at least 191, and no edge pixel is dropped.

That is the opposite of a floor, which darkens what a range brightens. The vendor applies the
range only in its Gray Scale Level mode: the Saturn 4 Ultra defaults to min 95 in another mode,
and its file carries greys down to 1, the file ADR 0139 matched.

## Decision

`core_raster::GreyRange { min, max }` replaces the floor in `Grey`, applied after the coverage
and the ladder by the formula above; `GreyRange::FULL`, 0..255, leaves every grey as it is.
The printer profile states it as `display.grey_min` and `display.grey_max`, 0 and 255 unless
stated, and a shipped machine states the vendor's default where its slicer defaults to the
mode that applies it. `--grey-range MIN..MAX` overrides the profile, as `--grey-floor` did,
and the window's printer form edits both ends. A range whose bottom is above its top is an
error in the profile and in the raster settings.

## Consequences

On the 19 machines, edges come out brighter and slightly fuller, as in the vendor's own files,
which its exposures are set for. Every other machine, the Saturn 4 Ultra among them, keeps
0..255 and the output of ADR 0139. A user's profile that stated `grey_floor` loses it: the key
is ignored, since a floor and a range do not convert. If a printed edge on one of the 19
comes out too heavy, the answer is that profile's `grey_min`, not a new global default.

## Alternatives considered

### Keep the floor and add the range beside it

Both act on the dimmest edge grey in opposite directions; two knobs for one end of the ramp,
with an order between them to explain, for a case no machine needs.

### State the range on every machine the vendor gives one

Applies 95 to the Saturn 4 Ultra, whose vendor file shows the range unused, and undoes the
match ADR 0139 measured.

### The option that won, and what it costs

The vendor's mode is read once, when the catalogue is kept by hand; a machine whose slicer
later changes mode keeps the range it shipped with until its profile is corrected.

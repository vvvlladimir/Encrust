# 0378. A machine may take masks without grey only

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

The vendor's slicer keeps anti-aliasing off, and hides the setting, for six machines we
ship: the Creality HALOT-X1, X1 Max, X1 Neo and SKY 2025, and the PIOCREAT PioNext Mini and
Ultra, all `.cxdlpv4`. Its machine profiles say so (`advanced.bantialiasing` hidden and
off). We offered grey on every machine, so a file for these carried masks no file from
the vendor ever does. Whether their firmware misreads grey is not known; no vendor file
with grey exists to compare against.

## Decision

`PrinterProfile.display.binary_only`, false unless stated, says a machine takes masks
without grey only; the six profiles state it. `core_pipeline::raster_settings`, the one
call the window, the command line and a converted file all draw their panel from, makes
the shading binary for such a machine whatever was asked, so grey levels, blur and floor
fall away with it. The window does not show *Edge smoothing* for it; the command line
warns when `--grey-levels`, `--grey-floor` or `--blur` is given for it.

## Consequences

Another such machine is a line in its profile. A plate or project made with grey on
another machine slices without it here, and goes back to its own settings on a machine
that takes grey, since the plate's choice is kept, only overruled. If a vendor file with
grey turns up for one of these machines, the line comes out of its profile.

## Alternatives considered

### A field of `locked`, beside the resin settings a machine pins

`locked` is a `PrinterTuning`, laid over the resin's own numbers; anti-aliasing is not a
resin setting, and would need a place in `MaterialProfile` it has nowhere else.

### Hide the setting in the window only

Leaves the command line and a converted file writing grey the window refuses, against the
rule that both reach the same core call.

### The option that won, and what it costs

A flag the window's printer editor does not offer: a hand-made profile states it in its
TOML or not at all.

# 0013. Exposure and motion settings belong to the material profile

- **Status:** Accepted
- **Date:** 2026-09-17

## Context

The `.goo` header and every layer definition carry settings `MaterialProfile` did not
have: retract distance, the bottom block's own lift distance and speeds, UV power for
bottom and normal layers, and the number of transition layers over which exposure ramps
from the bottom value to the normal one.

They had to come from somewhere. `PrinterProfile` describes the machine — panel geometry,
build volume, mirroring — and is shared by every resin. The new settings are not
properties of the machine: two resins on the same printer want different exposure,
different lift and different transition ramps.

The CLI also had no way to load a resin at all. `MaterialProfile` had a `Default` and no
loader, so step 3 never read one from disk.

## Decision

The new fields go into `MaterialProfile`, each with a `#[serde(default)]`, so a profile
written before this change still parses. `MaterialProfile` gains `from_toml_str`, `load`
and a `validate` that rejects a non-positive exposure or speed, mirroring
`PrinterProfile`.

Two derived values live there too, because they are properties of the resin and nothing
else needs to reimplement them: `exposure_of_layer_s(index)`, which applies the bottom
block and the transition ramp, and `is_bottom_layer(index)`.

`encrust-cli` grows `--material`. Without it, `MaterialProfile::default()` is used.
`assets/profiles/generic-resin.toml` ships as a starting point.

`--layer-height` becomes optional and falls back to the material's `layer_height_mm`. When
both are given the flag wins, and the writer records the height actually sliced at, not
the profile's.

## Consequences

A resin profile is now a real file a user can keep and share, and the exposure ramp is
computed in one place that both the header and every layer definition read.

`MaterialProfile` has grown to seventeen fields, most of which only the `.goo` writer
reads. It will grow again for `.ctb` in step 8. If it becomes unwieldy the split is by
purpose — exposure, motion — not by format.

Layer height now lives in two places that can disagree, the flag and the profile. The
resolution is one line in the CLI and one override in the writer, and both are tested, but
it is a seam to remember.

## Alternatives considered

### Put them in `PrinterProfile`

The printer's firmware does enforce limits on lift speed, so there is an argument the
machine owns them. Rejected: exposure is a property of the resin above all else, and
putting it on the printer would mean one profile per printer-and-resin pair.

### Hardcode sensible values in `GooWriter`

Smallest change: the format needs the fields, so fill them with what other slicers write.
Rejected: it makes a printable file impossible to tune without editing Rust, and step 8
adds a profile library that would have to undo it.

### The option that won, and what it costs

`printer-profiles` now carries settings that only exist because a file format asks for
them, which is a format concern leaking one crate down. The alternative — a third profile
type owned by `format-goo` — would leak the other way, since `encrust-cli` and the GUI both
need to show exposure to the user. Seventeen fields of resin settings in a leaf crate is
the cheaper of the two.

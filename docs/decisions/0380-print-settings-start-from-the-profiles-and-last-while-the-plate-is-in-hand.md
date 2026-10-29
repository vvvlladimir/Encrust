# 0380. Print settings start from the profiles and last while the plate is in hand

- **Status:** Accepted
- **Date:** 2026-10-29

## Context

Anti-aliasing, the edge grey levels and the edge blur were the window's own, one setting for
every resin, and the command line shaded every mask by coverage unless told not to. How far
an edge may be lit is the resin's business: a thin, fast-curing resin swells at a blurred
edge that a thick one needs.

The Print settings panel edits a copy of the printer's and resin's numbers: layer height,
adaptive layers, planes a layer, exposures and bands, edge smoothing, waits and motion. That
copy was remembered between runs (ADR 0192) and written into a project (ADR 0191), and
partly not: the bands and layer height travelled, the exposures, waits and motion did not.
A user could not tell which numbers a plate would slice with, nor whether an edit in the
panel would outlive it, and a resin re-measured in Settings did not reach a plate that kept
its old copy.

## Decision

The printer and resin profiles are the one statement of how to print. The panel's values
start from them, change the plate in hand only, and are dropped when a project is opened, a
new one started, a plate added or the window restarted. Choosing or editing a printer or
resin in Settings takes its values again, as before.

- `printer_profiles::EdgeSmoothing { anti_alias, grey_levels, blur_px }` is a resin's
  `[edges]` table, anti-aliased with every grey and no blur when absent, one for every
  printer the resin is on. `core_pipeline::PanelOverrides` is built from it, so the window,
  `core-engine` and the command line shade the same way.
- The project manifest, version 7, keeps none of them: `slicing` is island removal and the
  container. Versions 4 to 6 still read, their print settings ignored. `open_plate` takes
  the resin as the printer starts from it, with no bands and no adaptive layers.
- `ToolSettings` carries the panel's values for the undo stack alone, never serialised, and
  puts them back only on the printer and resin they were set on.
- The panel's help says that its values do not last and where to change them for good; its
  foot takes the profiles' back, enabled only when it holds anything else.
- The command line takes the resin's `[edges]` unless `--anti-alias`, `--no-anti-alias`,
  `--grey-levels` or `--blur` says otherwise. For a project as for a model, `--layer-height`,
  `--adaptive`, `--samples-per-layer`, `--exposure-at` and the mask flags apply.

## Consequences

What a plate prints with is always visible in two places, the printer and the resin, and a
re-measured resin reaches every plate it is chosen for. A project saved with a tuned exposure
no longer brings it back: the user tunes the resin in Settings instead, or the plate again.
A project of version 7 is refused by older builds, as any newer version is. If users ask
for a project to keep a plate's own numbers, that is a named, saved variant of the resin,
not a copy hidden in the project.

## Alternatives considered

### Keep the panel's values in the project

A project would slice as it was saved, whatever its resin became. It is what ADR 0191 did
for part of the values, and the part was the confusion: the same panel would hold numbers
that last and numbers that do not, and a stale copy would shadow a re-measured resin.

### Keep edge smoothing a window setting

No profile change, but a user with two resins has to switch it by hand each time, and the
command line cannot know which one a resin wants.

### Start from the profiles, and what it costs

A plate tuned by hand loses its tuning on the next open or restart. The panel's help says
so, and the way to keep them is the resin's own settings.

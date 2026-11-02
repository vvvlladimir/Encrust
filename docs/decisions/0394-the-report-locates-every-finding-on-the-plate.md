# 0394. Locate every finding of the JSON report in plate millimetres, in schema 1

- **Status:** Proposed
- **Date:** 2026-11-02

## Context

`--json` counts what is wrong — `trapped_pockets`, `islands`, `suction_cups`, `no_room_for`
— but says nowhere where, so a script or a language model driving the command line cannot
act on a finding (`docs/design/ai-integration.md`, step 1). The engine knows each place, in
three frames: `Trapped::at` and `bounds` in plate millimetres, a `Risk`'s `at_mm` in
millimetres off the first pixel of the mask it was measured on, its layer as an index into
the plan, and the supports' contacts in plate millimetres, without saying which got no
column. The window already turned a risk into a plate point, in a function of its own.

## Decision

Each count gains a list beside it, in schema 1, every field it had kept with its meaning:
`cured.risks`, `slicing.trapped` and `supports.no_room_at_mm`, wherever `Cured`, `Slicing`
and `Supports` appear. Every point is in plate millimetres from the plate's front left
corner after placement, the frame `--drain-at` takes. A risk's z is the bottom of its layer,
where it cures onto what is under it and where the window's Check stands a support under an
island; its layer counts from one, as the other `*_layer` fields do; its kind is kebab-case
and carries the kind's own readings. `docs/cli.md` lists the fields.

`RasterSettings::plate_mm` undoes a panel's mirroring, the inverse of the map the rasteriser
draws by, so the convention has one home: the CLI calls it with the panel the masks were
drawn for, the window with the picture it lays out. `core_engine::Stood::unheld` names the
contacts no tree holds, so `no_room_for` is that list's length.

No list is bounded. Each holds what `Measured` and `TrapScan` already keep in memory, a few
dozen bytes an entry; a consumer gating on a count still reads the count, and a list cut
short would hide the findings it most needs to act on.

## Consequences

An agent can drill at a trapped pocket's `at_mm`, support an island at its point, or turn
the model away from a lever, without reading the masks. The report of a model full of
islands grows with them, as its text already would without the cap of ten. A finding the
engine names later is located here too, or the additive promise breaks. The terminal's risk
lines still read the mask's own millimetres; a reason to reopen is a script reading those.

## Alternatives considered

### A new schema number

Clean for a reader that rejects unknown fields, but nothing in schema 1 changes meaning,
and a reader that ignores unknown fields — every JSON reader by default — keeps working.

### The point in panel millimetres, as `Risk` holds it

No conversion, but a mirrored panel puts it on the other side of the plate from the model,
and no flag takes that frame.

### Additive lists, and what they cost

Each finding is said twice, as a count and as a list, and the two must agree; the tests
hold them to it.

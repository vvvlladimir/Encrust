# 0171. A count read from a file is checked against the file

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

Every reader believed its header. A `.ctb` multiplied the panel's two fields into a `u32`
and panicked on the overflow; it also reserved a table row per claimed layer, so 110 bytes
asked for gigabytes. A `.pwmx` did the same for ninety-eight. A `.3mf` is a zip, and both
the reader under us and ours sized a buffer from the uncompressed length an entry states
about itself: a kilobyte of archive claiming four gigabytes was an allocation, not an
error. The fuzz targets of ADR 0161 found each one inside a minute, and `SECURITY.md`
promises a malformed file is refused rather than crashed on.

A count has two kinds of bound. One is the source itself: a header cannot hold more rows
than there are bytes behind them. The other is physical: a panel is as large as a panel
gets, and nothing in a file says so.

## Decision

Both bounds live in `core-format` and every reader goes through them.

- `Reads::claim(what, count, each_bytes)` refuses a count the source is too small to hold
  and answers the capacity to reserve. `Reads::bytes` refuses a length past the end before
  it allocates. The multiplication behind each is checked.
- `panel_in_range(width_px, height_px)` answers the pixel count, refusing an overflow and
  anything past `MAX_PANEL_PX`, 256 megapixels — about three times the largest panel sold.
  A PNG's own dimensions go through it too.
- `read_entry` reads one archive entry by growing, capped at `MAX_ENTRY_BYTES`, rather than
  reserving what the zip directory claims.
- `core-mesh-io` gains `zip` to read a 3MF's directory and refuse an oversized part before
  `threemf2` reserves from it.

Two new `FormatError` variants carry the claim and what was available: `ImpossibleCount`
and `PanelTooLarge`.

## Consequences

A hostile file costs a reader an error, not a process. Every count a container states is
now checked in one place, so a new `format-*` crate inherits the guard by using `Reads`
rather than by remembering to.

The panel ceiling is a number that will date: a panel past 256 megapixels would be refused
as hostile. That is the signal to raise it. `core-mesh-io` now carries `zip` for a check it
makes rather than for a format it reads, which is worth removing if `threemf2` stops
trusting the size an entry states.

## Alternatives considered

### Let each reader bound its own fields

Rejected: this is how the bugs arrived. Nine containers, each with its own idea of what a
sane count is, and a tenth about to be written.

### Cap the whole process instead

A global allocator limit, or a reader run in a child process. Rejected as out of
proportion: the fault is a believed header, and the fix belongs where the header is read.

### The option that won, and what it costs

`claim` needs each caller to say what one row costs in the file, and a caller that passes
too small a number gets a weaker bound than it could have. A count check against the whole
source rather than against the bytes behind the table is also looser than it could be; it
is what a single call can know without the caller seeking first.

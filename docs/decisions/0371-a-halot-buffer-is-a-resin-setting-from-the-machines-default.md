# 0371. A HALOT buffer is a resin setting that starts from the machine's default

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

The vendor slicer gives the newer Creality HALOT machines (X1, X1 Max, X1 Neo, SKY 2025,
MAGE PRO, MAGE S) and the PioNext Mini and Ultra a "buffer": an on/off switch, a height in
mm, a speed in mm/min and a light-off delay in s. On the HALOT machines it is on by default:
for example 5 mm at 120 mm/min, then 6 s on the X1 Max. It is a resin setting in the vendor's
profile (`material.slice.print`), with each machine's numbers as the default.

Three files sliced by the vendor for the X1 Max — buffer off, 3.7 mm / 97 mm/min / 4.3 s,
and 2.2 mm / 60 mm/min / 1.9 s — differ in sixteen bytes only: the last four words of the
68-byte print parameters, `f32` height, speed and delay, then `u32` 1 for on. Every layer's
motion block is the same in all three, so the firmware runs the buffer itself. We wrote
zeros there, which turned off a buffer the maker turns on.

## Decision

`Buffer { enabled, height_mm, speed_mm_min, light_off_delay_s }` lives in `printer-profiles`
and appears in three places:

- `PrinterProfile.buffer`: the maker's default, read off the vendor's profile. Only a
  machine that has it has the field, and the window offers the buffer only there.
- `MaterialProfile.buffer` and `PrinterTuning.buffer`: what a resin changes, kept per
  machine like every other tuned setting.
- `MaterialProfile::buffer_on(printer)`: the one rule a writer and the window follow. A
  machine without a buffer gets none; otherwise the resin's own buffer wins over the
  machine's.

The `.cxdlpv4` writer writes it, its reader reads it back, and a `.cfgx` resin import takes it.

## Consequences

- A HALOT file states the buffer its maker would, and a user can change it or turn it off.
- Nothing else carries the field. A machine whose format gains one adds its writer only.
- What the firmware does with each number is the maker's. We checked the bytes against the
  vendor's files, not against a print; `PLAN-encrust-profiles.md` §5 lists the check.

## Alternatives considered

### A printer setting with no resin override

Simpler, but the vendor's own profile holds the buffer per resin, and a resin that peels
badly needs a slower or longer buffer.

### A lock in `[locked]`

A lock is a default that the window hides (ADR 0301, 0364). The buffer has to stay visible.

### The option that won, and what it costs

One more optional field in three structures, and a form card shown only for eight machines.

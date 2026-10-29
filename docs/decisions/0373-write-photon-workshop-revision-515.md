# 0373. Write revision 515 of the Photon Workshop container

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

ADR 0166 left 515 unwritten, as no published machine profile asked for it. The Photon Mono,
Mono SE, Mono SQ and Ultra are 515 machines: it is the revision known files state
for their extensions (`docs/formats/anycubic.md`), and they were written as 516 until now.

## Decision

`AnycubicVersion::V515` is written, and `newest_version` picks it for `.pwmo`, `.pwms`,
`.pmsq` and `.dlp`. Every revision writes the same eleven tables; a reader takes what its
revision names and passes over the rest (`docs/formats/anycubic.md`).

## Consequences

The four machines get the revision their files carry, read back by our reader alike;
a file of theirs written before this states 516 and still reads here. Reopen on a 515 machine
that refuses a file of ours, or a known one that differs from it.

## Alternatives considered

### Keep writing 516 for them

What no known file carries for these machines, which only a print on each could show safe.

### The option that won, and what it costs

One more revision to keep in step with the others, and a test per machine that names it.

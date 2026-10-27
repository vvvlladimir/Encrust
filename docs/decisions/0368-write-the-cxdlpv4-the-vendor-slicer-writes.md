# 0368. Write the `.cxdlpv4` the vendor slicer writes, with its own codec and model codes

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

ADR 0168 wrote version 4 of the Creality container with the seven-bit runs of the `.ctb`
family, under `.cxdlp`, with the model code picked out of a `CL` or `CT` in the machine's
name. The vendor slicer's file for the HALOT X1 Max does none of that, and our reader could
not open it: its layers are six-bit runs that restart on every row, it is named `.cxdlpv4`,
its header names the machine `R012`, its layer rows state the cured area in whole mm², and
its header states the anti-alias level where we wrote one pass. The serialiser keeps a table
from machine name to code: `HALOT-X1 Max` is `R012`, `HALOT-ONE` `CL-60`, sixteen in all.

## Decision

`format-creality` writes and reads version 4 in that row codec, under `.cxdlpv4`, with the
vendor's fields (`docs/formats/creality.md`). `model_code` looks the machine name up in the
vendor's table first and picks a `CL` or `CT` code out of it second. A `.cxdlp` name still
opens either revision.

## Consequences

Our reader opens the vendor's X1 Max file and every pixel of it, and the HALOT X1, X1 Max,
X1 Neo, SKY 2025 and MAGE S can be profiled once their machine name is the vendor's. A file
of ours from before this, in the old codec, no longer reads. Grey is six bits, not seven.

A machine the vendor's table does not name and whose name has no `CL` code, such as the
HALOT PLAY and R6, still cannot be written. Reopen when a file for one of them is read.

## Alternatives considered

### A `model_code` field in the printer profile

General, but it would state twice what `machine_name` already says, and only the vendor's
table is known to be right.

### The option that won, and what it costs

The table is the vendor's data, kept by hand in our code, and grows only when someone reads a
newer vendor build. This supersedes the version 4 codec, extension and model rule of ADR 0168.

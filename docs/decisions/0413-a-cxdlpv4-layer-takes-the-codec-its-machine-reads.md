# 0413. A `.cxdlpv4` layer takes the codec its machine reads

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

ADR 0367 took the six-bit row codec from one known file, for the HALOT X1 Max, and wrote it for
every version 4 machine. A HALOT MAGE PRO (CL-103) then cured a sheet across the whole screen
and showed random horizontal lines. The machines' own software picks one of four layer codecs by
machine name; the X1 Max alone takes six-bit rows. The container says nothing of which is in use.

## Decision

The model code picks the codec: `LayerCodec::of_model`, table in `docs/formats/creality.md`.
Machines listed in no group take seven-bit runs over the whole image, the `.ctb` family's.

A layer is compressed away from the sink and with no access to it, so the writer is generic over
a marker type naming the codec, and `core-pipeline` picks the marker from the machine's name.

## Consequences

Each version 4 machine gets the bytes its firmware reads. Only the X1 Max codec is checked against
a known file; the other three come from static reading of the machines' software and are unproven
until such a machine prints. Grey is seven bits on most machines and eight levels on the paired
group. A file we wrote for another machine before this reads back wrongly or is refused, and
such a file must not be printed. Reopen when a file for one of the unproven groups is read.

## Alternatives considered

### Pass the codec to `LayerSink::encode`

Direct, but every format's sink would change for one container's need, and the sink-free call
is what lets layers compress on other threads.

### Compress in `push`

No generics and no marker types, but it moves work proportional to layers onto one thread.

### The option that won, and what it costs

Four marker types and a four-armed match in `core-pipeline`, a small price while the codecs are few.

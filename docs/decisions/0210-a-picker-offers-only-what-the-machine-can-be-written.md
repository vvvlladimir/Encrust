# 0210. A picker offers only the containers the machine in hand can be written

- **Status:** Accepted
- **Date:** 2026-10-08

## Context

Every container the window writes was on every picker, whatever machine was in hand. Most
of them ask nothing of a profile, but the `.cxdlp` header carries the machine's own `CL`
or `CT` code and the firmware matches that and nothing else, so `format-creality` refuses
a profile whose name has none rather than guess one a machine would not recognise.

The refusal came at the end of a run: the stack was cut, the panel rasterised, and only
the writer's first call said the file could never be written. A user on a machine of
another make had no way of knowing the choice was closed to them before they made it.

## Decision

`SlicedFormat::writable_for(&PrinterProfile)` answers whether a profile carries what the
container's header needs, and `SlicedFormat::choices_for` is the list a picker walks.
Today only the `.cxdlp` pair answers `false`, by asking `format_creality::model_code` of
the machine name. The format in hand goes through `Slicing::set_format`, which falls back
to the default when a profile or a restored session names one this machine cannot take.

## Consequences

A machine is never offered a container it cannot be written, and the Slice button never
arms a run whose only ending is the writer's refusal. The refusal itself stays: a name
typed with a `.cxdlp` extension still decides the format (ADR 0047), and a hand-written
profile can still carry a container of another family.

`core-pipeline` now asks a format crate a question about a profile. The signal to revisit
is a third such question: two is a pair of match arms, more is a method on the writer
trait.

## Alternatives considered

### Write a model code when the name carries none

A file would come out of every machine. It would not print on any of them, because the
firmware reads the code it was given and stops. A file that cannot print is worse than a
refusal, which at least says what to change.

### Leave the picker alone and only reword the refusal

Cheapest, and it does tell the user to put `CL-60` in the machine name. But it still
spends a full slice to say so, and it leaves a choice on the list that is wrong for every
machine of another make.

### The option that won, and what it costs

`core-pipeline` carries a match arm naming one format's requirement, which is the kind of
enum switch the architecture rules push back on. It is one arm against one real case, and
the cost of the alternative — a `writable_for` on `SlicedFileWriter`, reached through a
writer built for a job that does not exist yet — is an abstraction ahead of its second
use.

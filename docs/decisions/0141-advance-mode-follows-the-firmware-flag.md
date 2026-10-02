# 0141. Advance mode follows the machine's firmware flag

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

ADR 0090 said `.goo` sets advance mode "whenever a job's exposure varies at all", which
`PrintJob::varies_by_layer` reads as a transition ramp, an exposure band or a non-uniform
stack. A ramp alone is enough, and almost every job has one, so almost every file we write
claims advance mode.

A vendor file for an Elegoo Saturn 4 Ultra claims the opposite. It carries the same
ramp in its per-layer records, the same `transition layers` count in its header — and
advance mode `false`. The machine ramps the bottom block from the header's bottom exposure,
common exposure and transition count, and never reads the records.

So the flag is not a statement about the stack; it is a request that the firmware take a
different path through the file. `PrinterProfile::firmware.per_layer_settings` already says
whether a machine honours that path, but only `validate` consulted it, to refuse a banded
job. Nothing stopped us asking a machine for the per-layer path on a job that did not need
it.

## Decision

`varies_by_layer` is `per_layer_settings && (a ramp, a band or a non-uniform stack)`. A
machine whose profile says it reads the header alone is never sent a file claiming advance
mode, whatever the stack does. A job that genuinely needs the tables — an exposure band, an
adaptive stack — is still refused on such a machine by `validate` rather than written
silently at header exposure.

Both firmware flags get a switch in the printer form, beside the panel and the build volume,
because they are the two things about a machine that no file format states and only a print
reveals.

## Consequences

The default is unchanged: `per_layer_settings` still defaults to `true`, so every shipped
profile writes what it wrote before. What changes is that a user with a machine that
misbehaves on the per-layer path can turn it off and get the vendor's bytes, without editing
TOML.

The per-layer records are still written in full either way — the format fixes their size, so
there is nothing to save by leaving them out, and a machine that ignores them loses nothing.

The signal to revisit is a machine that needs advance mode set for a stack that varies in
none of the three ways above, which would mean the flag carries something else again.

## Alternatives considered

### Drop the transition-ramp clause outright, matching the vendor file

Would make an ordinary job byte-compatible with the reference file on this flag. It lost
because the ramp in the per-layer records is real and some machine may well be reading it;
turning it off for every machine on the evidence of one vendor's file for one machine is a
wider claim than the evidence supports.

### A per-job switch in the Slicing panel

Reachable where a user is already choosing exposure. It lost because the flag is a property
of the machine, not of the job: the same answer is right for every file that machine prints,
and a per-job switch is a setting to get wrong every time.

### The option that won, and what it costs

A switch that most users should never touch, on a form that is already long, describing a
distinction — header numbers against per-layer numbers — that means nothing until a print
fails. It also leaves the default as the value the one file we can check disagrees with.

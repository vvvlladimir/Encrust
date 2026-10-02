# 0142. A tilting vat reads the header alone

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

ADR 0141 made `.goo`'s advance mode follow `PrinterProfile::firmware.per_layer_settings`,
which defaults to `true`. The Elegoo Saturn 4 Ultra profile carried no `[firmware]` block,
so it took that default and every file we wrote for the machine claimed advance mode.

A print from such a file stopped rising at around 0.6 mm — the first normal layers, just
past the bottom and transition block — while the panel kept exposing. The same job with
`per_layer_settings = false` printed to the end. The two files differ in one meaningful
byte, offset 195445, and in nothing else: same layer count, same Z per layer, same
exposure ramp in the records, same lift and retract. A vendor file for the machine
carries the same records and clears the same byte.

The Saturn 4 Ultra separates a layer by tilting the vat, not by pulling the plate off the
film. Elegoo's own material says lift height and speed stop being meaningful on such a
machine, and readers zero them for it. Our per-layer records carry the profile's
5 mm lift and 5 mm retract, which are inherited from peel machines and are not a motion
this one can perform. Advance mode is what makes the firmware obey them.

## Decision

`elegoo-saturn-4-ultra.toml` sets `per_layer_settings = false`. A machine that separates
by tilting its vat reads the header alone, and its profile says so.

The default stays `true`: it is right for the peel machines, which are still most of them,
and a tilting vat is a property of one machine rather than of a manufacturer or a format.

## Consequences

Files for this machine are byte-compatible with the vendor's on the flag. The bottom block is
ramped by the firmware from the header's bottom exposure, common exposure and transition
count, which is what the working print did.

An exposure band by height, and an adaptive stack, are now refused for this machine by
`core_format::validate` rather than written into records it ignores. That is the honest
answer — the machine cannot execute either — but it is a capability the profile loses.

The per-layer records still carry the profile's lift and retract, now as dead weight. If a
second tilting machine arrives, or if a future firmware reads the records on this one, the
right move is a `tilting_vat` flag that zeroes those fields, not a third copy of this
decision.

The signal to reopen: a Saturn 4 Ultra that prints a banded job correctly with advance mode
set, which would mean the stall was the motion values rather than the path through the file.

## Alternatives considered

### Zero the lift and retract in the records instead

Keeps advance mode, and so keeps exposure bands, while removing the motion the machine
cannot perform. It lost because it is a guess about which field stalled the plate: the
evidence names the flag, not the numbers, and a second failed print on real resin is an
expensive way to narrow it.

### Default `per_layer_settings` to `false` for every Elegoo

Would have caught this machine without a per-profile edit. It lost because the Mars 3 Pro
and Mars 4 Ultra are peel machines with no reported trouble on the per-layer path, and
turning it off for them on this evidence is a wider claim than the evidence supports.

### The option that won, and what it costs

One machine loses banded exposure and adaptive layers outright, with an error rather than a
degraded file. It also leaves the shipped default disagreeing with the only two working
files we have for any Elegoo, so the next tilting machine added will fail the same way
until someone prints and finds out.

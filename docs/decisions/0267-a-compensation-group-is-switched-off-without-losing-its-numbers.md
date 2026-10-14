# 0267. Switch a compensation group off without losing its numbers

- **Status:** Accepted
- **Date:** 2026-10-14

## Context

A resin's `Compensation` applied every non-neutral field it held. The only way to print
once without the shrinkage or the wall offsets was to type the neutral values back in, and
lose the calibrated ones. The Compensation card is being redrawn as three groups under a
switch each — shrinkage, tolerance, bottom tolerance — and a switch has to be stored
somewhere. Resin profiles written before this change carry no switch.

## Decision

`Compensation` carries three booleans: `shrinkage_on`, `tolerance_on` and
`bottom_tolerance_on`. A group switched off keeps its numbers and moves nothing:
`scale` is the identity without `shrinkage_on`, and `offsets_of_layer_mm` gives zero for
a layer whose group is off, the parity offset included. Every switch defaults to on, in
`Default` and for a field missing from a file, so a profile written before the switches
prints exactly as it did. The layer time has no switch: it moves only the estimate.

## Consequences

- A calibration survives a print made without it.
- A fresh resin shows its switches on over neutral values, which change nothing.
- `PrinterTuning` overrides the switches with the rest of `Compensation`, as one value.

## Alternatives considered

### A switch in the window alone, zeroing the fields

No format change, but switching a group off throws the calibration away, which is what the
switch is there to avoid.

### The option that won, and what it costs

Three more fields in every resin file, and a missing field reads as on rather than as the
struct's own value, so the two defaults have to be kept in step by hand.

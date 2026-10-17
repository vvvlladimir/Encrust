# 0296. State the `.sl1` fade as the bottom block and the ramp together

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

ADR 0148 took `numFade` for our transition layers: the writer stated the ramp alone and the
reader read the count back as bottom layers. The firmware does neither. It exposes layer 0
for `expTimeFirst`, steps down linearly until layer `numFade - 1`, which takes `expTime`,
and divides by `numFade - 1` on the way. A resin of six bottom layers and no ramp was
written as `numFade = 0`, and a file with `numFade = 10` was read as ten bottom layers
where the machine prints one and a ramp of eight. The vendor's slicer keeps the count
between 3 and 20.

## Decision

The reader states one bottom layer and a ramp of `numFade - 2`, which exposes every layer
as the firmware does. The writer states `bottom_layers + transition_layers + 1`, held to 3
to 20, so the normal exposure starts on the layer it does in our resin. An imported
material INI's `faded_layers` is read the same way.

## Consequences

A file read back prints as the machine prints it, and one we write never states a count the
firmware divides by zero over. A bottom block of more than one layer is written as a ramp,
which the container cannot express any other way; such a block is a little underexposed
after its first layer. Reopen if a firmware is found that holds a block before fading.

## Alternatives considered

### Refusing a resin with more than one bottom layer

Faithful, but every resin would be refused, since none holds a single bottom layer.

### Keeping the count as the ramp alone

What ADR 0148 did: the normal exposure then starts before our ramp is over, and a resin
with no ramp writes a count the firmware divides by zero over.

### The option that won, and what it costs

A bottom block is faded rather than held, and a written file read back states one bottom
layer and a longer ramp than the resin it came from.

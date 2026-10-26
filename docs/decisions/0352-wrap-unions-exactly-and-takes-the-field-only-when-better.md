# 0352. Wrap unions a model's shells exactly, and takes the field only when it is better

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

Some models cannot be repaired face by face: a scan or a kit of parts pushed into each other
has shells that cross, faces inside other shells and surfaces that cross themselves. What
prints is the outer surface of all of it. `core-volume` already joins models by the exact
engine and falls back to a sampled field (ADR 0304, 0321), and repair already runs in
`core-geometry` (ADR 0349, 0351).

Measured with `encrust inspect --repair=wrap --intersections` on the test models:

| Model | Before | Field result | Time |
|---|---|---|---|
| statuette, 1 shell | 3 crossing pairs | 9 pairs, 32 branching edges | 10 s |
| calibration set, 15 shells | 27 392 pairs | 94 pairs, 209 branching edges | 5.7 s |
| colour mix, 55 shells | 827 325 pairs | 2 105 pairs, 3 080 branching edges, 361 shells | 122 s |

The engine reads a shell that crosses itself and returns it still crossing. The field's
surface pinches parts thinner than its lattice into branches.

## Decision

**`core-volume::wrap` repairs the mesh, then unions its shells with the exact engine, and
keeps the result when it is sound and crosses nothing.** Otherwise the repaired mesh is
sampled into a winding-number field on `wrap_mm` (0.2 mm by default, `--wrap-mm`) and its
surface extracted. **That surface is taken only when it is sound, crosses itself less than
the repaired mesh and branches no more**; else only the repair stands, and `Repaired::wrapped`
is false so the caller can say the wrap was left out.

`RepairMethod` in `core-volume` is a `MendMethod` of `core-geometry`, `Wrap` or `Deep` —
repair, wrap, repair — and `core-volume::repair` runs any of them. The command line names
them as `--repair=wrap` and `--repair=deep`. The window gains them with the Repair tool.

A shell inside another is not kept as a cavity: repair turns every closed shell outward
(ADR 0269), so it is a body, and the wrap swallows it.

## Consequences

- Separate closed shells that overlap become one surface, exactly, with no face moved but
  where they cross.
- On every test model the field result was worse and the wrap was left out. Wrapping a
  surface that crosses itself waits for a field surface that does not branch.
- A refused wrap still costs its field: two minutes on the colour mix.
- The signal to revisit is an extraction that keeps thin parts manifold, or an engine that
  resolves a shell crossing itself.

## Alternatives considered

### Take the field's surface whenever the exact union fails

What `combine` does. Rejected for wrapping: on every test model it left more crossings or
branches than it removed, which is worse than the repair alone.

### Keep a shell wound inward as a cavity

What the plan asked for. Rejected for now because no shell reaches the wrap wound inward:
orientation turns each closed shell outward on import and in repair, and changing that is a
decision about orientation, not about wrapping.

### The option that won, and what it costs

A wrap that, on today's models, mostly says it was left out, and a check of crossings on the
field's result that costs a hierarchy and a pass over its faces.

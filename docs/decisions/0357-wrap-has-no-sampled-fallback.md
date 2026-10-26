# 0357. Wrap unions exactly or leaves the repair alone, with no sampled fallback

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

ADR 0352 has `wrap` fall back to a winding-number field on `wrap_mm` where the exact union
fails, and take its surface only when it is sound, crosses itself less and branches no
more. Its own consequences say the field never passed that check and still cost its build.

Measured with `encrust inspect --repair=wrap` on all 31 test models, after the field's
sign was made cheaper: 22 wrapped exactly, 9 were left out, none took the field. The field
was the bulk of every refusal: 34 s of 45 s on the colour mix (4.1 M faces, 55 shells),
2.9 s of 4.5 s on the statuette. The extraction pinches parts thinner than the lattice into
branching edges, which is why the check refuses it.

## Decision

**`core-volume::wrap` unions the repaired mesh's shells with the exact engine and keeps the
result when it is sound and crosses nothing; otherwise only the repair stands.** No field
is built. `RepairSettings::wrap_mm`, `Repaired::voxel_mm`, `--wrap-mm`, the window's wrap
lattice and the batch report's `wrap_voxel_mm` go with it.

Before the engine, a mesh of one sound shell crossing nothing is its own wrap, and a shell
crossing itself skips the engine, which returns it still crossing (ADR 0352). A deep repair
whose wrap is left out stops there: its second repair would meet the first one's mesh.

## Consequences

- A refused wrap costs the repair, one pass for crossings and at most the engine: a deep
  repair of the colour mix goes from 45 s to 6.3 s, of the statuette from 4.5 s to 1.1 s.
- A shell crossing itself is never wrapped. A synthetic one with no thin part, a cube
  pulled through its own floor, was the only body the field ever helped.
- Saved window settings carrying `wrap_mm` read as before; the field is ignored.
- The signal to revisit is an extraction that keeps thin parts manifold, or a wrap that
  guarantees a surface without branches; either brings back a fallback under a new ADR.

## Alternatives considered

### Keep the field and its check (ADR 0352)

Rejected: on no test model did it change the result, and it was most of the time a wrap
took whenever the union failed.

### Keep the field behind an option, off by default

Rejected: a setting no model benefits from is a control to explain and code to keep for a
case that does not exist yet.

### The option that won, and what it costs

A model whose shell crosses itself and has no thin part, which the field could have wrapped,
now keeps only its repair; the user can still cut or combine it by hand.

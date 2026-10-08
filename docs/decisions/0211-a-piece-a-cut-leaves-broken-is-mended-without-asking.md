# 0211. A piece a cut leaves broken is mended without asking

- **Status:** Accepted
- **Date:** 2026-10-08

## Context

ADR 0194 settled that a hole is closed only when the user asks for it, because a patch is
surface the file never contained: a lampshade modelled open and a scan with a hole are the
same mesh to us. A model that lands broken therefore raises a question, and the answer
decides what is on the plate.

A piece that comes off the Cut tool is not that model. Its surface over the plane was
invented by this program a moment ago, and where that cap does not close — a model that
touches itself on the plane, a half of a model that was already open — the user is asked
about a hole they did not make, in a model they only asked to cut. Cutting twice over was
enough to meet it: the second cut tore its section on the first cut's cap, and both
pieces came down carrying `broken`.

The tear itself is a bug and is fixed in `core-geometry`. What is left is the question of
what the window does when a cut still comes out unsound, which no amount of fixing the cap
makes impossible: the model being cut may be open to begin with.

## Decision

**A piece a cut or a split puts on the plate is mended on the way down if its diagnostics
are not sound, and nothing is asked.** `encrust-app::cut::place` diagnoses the mesh,
runs `job::repair::mend` — the pipeline the Repair job already runs, now shared — and
diagnoses again, so what the piece carries describes the mesh that landed.

**What mending did is said in the status bar, not on the model's row.** The Cut status
gains `; one piece was mended to close it`. The piece's summary keeps the default of a
model with no file behind it, which is what ADR 0194 settled for a repaired import.

**A piece mending cannot close keeps its mark.** The diagnostics are taken again after
mending, so a piece that is still open still reads `broken` and still holds the Slice
footer's reminder.

## Consequences

- The Cut tool's product is sound or says why not, with no modal between the press and the
  plate. Undo takes the whole thing back, mending and all, because it is one edit.
- Mending runs on the window's thread, where an import's runs on a worker (ADR 0038). It
  runs only on a piece that is already broken, and the cut that produced it ran there too.
  The signal to revisit is a model big enough that the mend is felt in the frame.
- A model cut while it is open gets its tear patched flat as a side effect of being cut,
  without being asked. That is the honest cost of the decision; the status bar says it
  happened and undo takes it back.
- `mend` is now called from two places, so the repair pipeline has one definition and the
  Repair job is a thread around it.

## Alternatives considered

### Ask about a cut piece the way import asks

Consistent with ADR 0194 and no new behaviour. Rejected because the question is about
surface the user did not supply: they pressed Cut, and the only answer that gets them what
they pressed it for is the one the window can take itself.

### Spawn the Repair job for each broken piece

Keeps the window's thread free and reuses `Repairs` whole. Rejected because the pieces land
first and mend afterwards, which puts a second mutation after the edit undo recorded, and
leaves the plate holding models marked broken for as long as the job runs.

### This decision, and what it costs

A flat patch over a hole in a cut piece is as crude as any other flat patch, and it is
applied without the user seeing the hole first. It is applied to a piece they can see on
the plate, the status bar names it, and undo is one key away — which is what makes doing it
silently affordable, not what makes it free.

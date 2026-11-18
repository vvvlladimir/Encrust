# 0412. Read a model into the engine once for its drill and its plugs, and build its shell on that read

- **Status:** Proposed
- **Date:** 2026-11-07

## Context

A hole drilled in the window starts two runs off the drawing thread: the drill, which takes
the cuts out of what the model prints from exactly (ADR 0322), and the plugs, cut from the
model round its holes (ADR 0321). Each read its own mesh into the engine, mended where the
engine refused it, and on a dense model that read is nearly all the work. On a 4.1-million-face
open model of 55 shells, hollowed to 1.5 mm with 17 holes, the plugs spent 9.4 s reading the
model and 0.25 s cutting; the drill read the shell — the model with its cavity appended — in
2.4 s, mended it in 4.3 s, read it again in 5.2 s and was refused, leaving the cuts to the
winding rule. Neither run told the window how far it had got, so for ten seconds nothing moved
and the plugs then landed at once.

## Decision

`core_volume::ModelRead` holds one model and its read, made on the first ask and waited on by
a second (`OnceLock`). The window keeps one a mesh, weakly, while a drill or a plug run holds
it (`drilling::ModelReads`), and hands it to both. A project opened by the engine, and
`encrust-cli` for its supports and its file, drill through the same call on a read of their
own: the command line takes the model from the faces that begin an inward shell.

The plugs crop that read rather than reading the model. The drill builds a hollow model's
shell on it: the model less its cavity read on its own, and its infill unioned back, which is
what the positive winding rule prints. A shell that does not begin with the model byte for
byte — a mould's, an opened floor's — or whose cavity the engine refuses is read whole, as
before, and that run counts its own read. A model refused is not read again.

The read counts three pieces — as it stands, mended, mended read — on a `Progress` of its own;
each run reports Reading while it waits on it, then the plugs count one a plug and the drill
its boolean and its mesh. The stage notice shows one card for both at the share of the one
least far along. A model's plugs are held until its drill ends and land then, so the card
ends before anything they note.

Its Cancel drops both runs; each stops at its next step — a read under way ends first — and
nothing is started again for the same cuts: the drill's are kept as refused, left to the
winding rule, and the model is marked as laid for the plugs asked, keeping those it has.
Plugs already cut and held, or waiting on the cut, go with them.

## Consequences

The model is read once, and the engine reads a cavity — a clean surface of its own — rather
than the whole shell. On the model above the plugs land at 8 s rather than 10, and the drill,
refused before, takes the holes out exactly in 15 s. A model the engine reads as it stands
still costs one read, now shared.

The whole model read is held while either run holds it and let go when both end; it is not
kept for the next edit, as it was not before. A plug run started after the drill has ended
reads again.

Reading has no finer count than its three pieces, so the bar steps by fifteen points through
it.

## Alternatives considered

### One run that drills and cuts the plugs together

One read by construction, but the two are started by different edits — a gap moved cuts only
the plugs, a shell rebuilt drills only — and land into the history apart.

### Cut the plugs from the shell read

The shell is what the drill reads, but it is the read the engine refused on the model above,
and its plugs would then have been sampled.

### The option that won, and what it costs

A run may wait for the other's read rather than start its own, and a shell built on the model
can differ from one read whole where the mended model and the mended shell were patched
differently.

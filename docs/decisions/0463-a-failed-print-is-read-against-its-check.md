# 0463. A failed print is read against the check of the file printed

- **Status:** Proposed
- **Date:** 2026-11-18

## Context

A recipe prepares a model; nothing in Encrust learns whether the print came out. A chat
client is where a user says what went wrong, often with a photo, and a language model asked
"why did it crack" answers from memory: plausible, untested, and blind to what the engine
knew of the file — a pocket of trapped resin, a suction cup at layer 59. Each job's revision
keeps the `--json` report of its check (ADR 0455), which holds those findings located.
`encrust-mcp` depends on no crate of the workspace and drives the command line.

## Decision

- **Symptoms from a fixed list**, named by the client from the user's words or photo:
  `off-plate`, `layer-split`, `hole`, `support-pulled`, `face-mark`, `resin-crack`. No image
  reaches the engine.
- **A map in data**, `crates/encrust-cli/src/diagnose/failures.toml`: each symptom's causes,
  each with its step, the change to try and an optional sign read from the report. The map
  lives with the command line because the report is the command line's schema.
- **`encrust diagnose REPORT --symptom S…`** reads a `--json` report and answers each
  symptom's causes: those whose sign the report shows first, with what it shows, then those
  no check can see; one whose sign is absent is left out.
- **`report_print`** runs it against the check of the revision last written (or the one
  named), and appends the symptoms, the points and the user's words to the job's
  `prints.jsonl`. The change to try is made with the editing tools, checked and compared as
  any edit.

## Consequences

A client's advice after a print starts from what the engine found in the file, and the map
is tested and changed like code: a print the map misread is a line in a TOML file, not a
prompt. The answer is only as good as the map's few causes; the exposure and the printer,
which no report sees, stay general advice. The prints kept are the evidence for improving
the map, and for `agent-lab` to grow sets from (step 21). Reopen when the window offers the
same, or when photos prove readable by a test.

## Alternatives considered

### The client reads the photo and decides alone

Nothing to build — but nothing to test, and the client never sees the report's findings
unless it asks for them and knows what they mean.

### The map in `core-engine`, over the engine's types

The plan's first place for it. The engine's findings reach a client only through the
command line's report, and the window does not diagnose yet; a map over the report serves
both callers there are, and moves down when the window needs it.

### The option that won, and what it costs

A second list of symptoms in `encrust-mcp`'s schema beside the command line's, kept the same
by hand — a name the command line does not take is a tool error — and a map whose ranking
is judgement until prints correct it.

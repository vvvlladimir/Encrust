# 0400. Bench a language model through its own client, against the autopilot

- **Status:** Proposed
- **Date:** 2026-11-03

## Context

A model's path through the tools is worth offering only where it matches or beats what the
engine's autopilot (ADR 0399) does alone, so both must be scored on one model by one rule. A
model reaches Encrust through a chat client over `encrust-mcp` (ADR 0397): the client runs the
agent loop on the user's own plan, and each client has its own prompt handling, tool
permissions and output. The tool calls a model makes and the tokens it spends are part of
its cost, and only the server sees the calls; only the client knows the tokens.

## Decision

**`agent-lab`**, a bench binary in `encrust-cli` beside `orient-lab` and `support-lab`, runs
each model of a corpus through `encrust --json estimate --prepare` and then through each
client under test, given as `NAME=COMMAND`: the command line of the client itself, split at
whitespace, its placeholders filled with one fixed prompt and an MCP configuration that starts
`encrust-mcp` with a job directory and trace of its own. The client's job's latest revision is
its answer.

`encrust-mcp` takes `--trace FILE` and appends one JSON line per tool call — tool,
milliseconds, failed, and the job and revision it answered with — and keeps the report of each
revision's check beside its recipe as `N.json`, so the bench scores exactly what the model saw.
Tokens are the last `usage` object the client prints on stdout.

Both are scored on the report — status, islands, suction cups, trapped pockets, contacts with
no room, resin, print time — and a model matches the autopilot when no status or count is
worse and resin and time are within a tenth above it; better on one reading as well, it
beats it. `--json` keeps a run and `--baseline` fails one where a model that held no longer
does, or where the autopilot is worse than it was.

## Consequences

Any client that takes an MCP configuration and a prompt on its command line can be benched
with no code here and no API key in the repository, on the plan its user already pays for. A
trace is a file format two binaries share: a field is added, never renamed. The autopilot's
flags are written in the bench as the server writes them for a recipe that only prepares —
two copies of one line, since `encrust-mcp` depends on nothing here. Tips on faces looking up
are not in the report, so not in the score yet. A client killed at its time limit may leave
its server running. Reopen when the window's agent panel (step 9) runs its own provider: it
can be benched in process.

## Alternatives considered

### The bench calls a provider's API itself

One loop for every model, tokens counted exactly — but it needs an HTTP client, a key and a
tool loop of its own now, benches a loop no user runs, and is step 9's work.

### Counting calls from the client's output

No change to the server — but every client prints its transcript its own way, and one prints
none.

### The option that won, and what it costs

Each client is driven through its own command-line quirks — `claude -p` takes the prompt before
the flags that take lists — and a run spends the tokens of a real session.

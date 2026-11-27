# 0455. A chat client drives the command line over MCP

- **Status:** Accepted
- **Date:** 2026-11-02

## Context

Users want a language model in a chat client to take a file through repair, pose, hollow,
drains, supports and the check to a printer file. The Model Context Protocol is how such
clients reach a local program; a subscription reaches a program only through its client, never
through an app's own login. `encrust` already does every step, with `--json`, exit codes and
`--strict` as a contract (ADR 0176). On stdio a server's stdout is the protocol. A model's way
through the tools is worth offering only where it matches or beats the autopilot (ADR 0402),
scored on one model by one rule.

## Decision

- **`encrust-mcp`** serves MCP over stdio with the SDK `rmcp` on `tokio`. Each tool runs
  `encrust --json` as a child — `--encrust PATH`, else the binary beside it, else the `PATH` —
  with a time limit and a progress beat, and answers with the report; `render` answers with
  PNG images in base64. Its only workspace edges are `core-edit` (ADR 0456) and `agent-link`.
- **A job** is a directory of numbered revisions, each the server's TOML of what was asked,
  kept beside its check's report (`N.json`), flags (`N.args.json`), recipe (`N.recipe.toml`),
  edits (`N.edits.json`) and plate (`N.encrust`). An edit is kept only if the command line took
  it, under a lock per job; `revert` copies one forward; `write_file` slices under `--strict`
  unless the user accepted the defects. Paths must be absolute, no name may start with `-`.
  The instructions tell the model to ask for the printer and resin, never to choose one.
- **`agent-link`** holds what the server and the window say to each other and where: the
  link's lines and answers, `WindowFile`, `AgentFile`, and the folders under `cache_dir`. It
  depends on `core-edit`, `directories` and `serde` alone.
- **`agent-lab`**, a bench in `encrust-cli`, runs each model of a corpus through
  `estimate --prepare` and through each client under test, given as `NAME=COMMAND` with an MCP
  configuration of its own; `encrust-mcp --trace FILE` logs each tool call. Both are scored on
  the report — status, islands, cups, trapped pockets, resin, time — and a client matches the
  autopilot when nothing is worse and resin and time are within a tenth.
- **Each release ships an `.mcpb` extension** per installer's platform,
  `encrust-<os>-<arch>.mcpb`, packed by `cargo xtask mcpb --target` with the `zip` crate and
  signed where `MCPB_CERT` and `MCPB_KEY` are set. The window's File menu opens the one for its
  version and system in the browser; opening it installs it in the chat app.

## Consequences

Every tool is the command line, so a fix to one reaches both, and a crash ends a child, not the
server. Each check stages the model again, so a chain of edits pays for hollowing each time.
`tokio` and `rmcp` stay out of `workspace-hack` (ADR 0272). Any client that takes an MCP
configuration and a prompt on its command line is benched with no API key in the repository. A
trace and a job folder are formats two binaries share: a field is added, never renamed. A new
release is a new download; nothing updates the extension in place. Reopen when the window serves
the tools itself, or when the extension directory takes listings again.

## Alternatives considered

### The command line in process, through its library

No second process — but `encrust_cli::run` prints to stdout, the protocol here, and a panic
would end the server.

### The bench calls a provider's API itself

Tokens counted exactly — but it needs a key and a tool loop of its own, and benches a loop no
user runs.

### The window writes the chat app's configuration itself

No download — but the window would edit another program's file and holds no `encrust-mcp`.

### The option that won, and what it costs

Every edit waits on a full stage, one more binary is built per target, and each client is
benched through its own command-line quirks, spending a real session's tokens.

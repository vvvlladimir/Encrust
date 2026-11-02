# 0395. A language model drives the command line over MCP

- **Status:** Proposed
- **Date:** 2026-11-02

## Context

Users want a model in a chat client — Claude Desktop, Claude Code, Codex, Cursor — to take a
file through repair, pose, hollow, drains, supports and the check to a printer file. The
Model Context Protocol is how those clients reach a local program; a subscription's plan
reaches a program only through such a client, never through an app's own login. `encrust`
already does every step, with `--json`, exit codes and `--strict` as a contract a script can
rely on (ADR 0176). MCP 2026-07-28 keeps no session: each request names what it acts on. On
stdio a server's stdout is the protocol, and the command line prints its report there.

## Decision

**`encrust-mcp`**, a binary crate depending on no crate of this workspace, serves MCP over
stdio with the official SDK `rmcp` 3.8 on `tokio`. Each tool runs `encrust --json` as a child
process — `--encrust PATH`, else the binary beside its own, else the one on the `PATH` — with
a time limit and a progress beat to the client, and answers with the report.

A **job** is a directory of numbered revisions, each a TOML **recipe**: the model, the printer
and resin, and each step's settings, which become `encrust` flags given as `--flag=value`. A
tool that edits checks the changed recipe with `estimate` and keeps it as the next revision only
if the command line took it, under a lock per job; `revert` copies an old one forward;
`write_file` slices under `--strict` unless the user accepted the defects, and a refusal is a
tool error. A call names its job, so nothing lives in the process but the locks. Paths from the
client must be absolute and no name may start with `-`. The instructions tell the model to ask
for the printer and resin, never to choose or invent one, and to ask before writing or sending.

## Consequences

Every tool is the command line, so it can do no less and no more than `encrust`, and a fix
to one reaches both. A crash or a runaway hollow ends a child, not the server. Each check
stages the model again from its file, so a chain of edits pays for hollowing and supports
each time; a model that takes a minute to hollow takes a minute a call. `tokio` and `rmcp`
enter the lockfile for this binary alone: hakari leaves it out of `workspace-hack`, so no
other crate is built with their features (ADR 0272). Reopen when the window serves the tools (the
plate already in memory), or when a client needs results streamed.

## Alternatives considered

### The command line in process, through its library

No second process and no reparse — but `encrust_cli::run` prints to stdout, which is the
protocol here, and a panic in a run would end the server.

### A `.encrust` project as each revision

It holds the shells and trees built, so a check would not rebuild them (ADR 0191) — but the
command line writes no project, and slicing one refuses every flag that shapes a model, so
the tools could not edit it.

### A crate of tools shared with the window first

The window's agent panel will run the same catalogue on its own plate. Shared now, it would
be an abstraction with one user; it moves down when the panel needs it.

### The option that won, and what it costs

A recipe is flags, so what the window can do and the command line cannot is out of reach,
and every edit waits on a full stage.

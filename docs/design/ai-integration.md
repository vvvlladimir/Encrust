# AI integration

How a language model takes a model from its file to a checked printer file through Encrust:
in Claude Desktop, ChatGPT and other chat clients, and later from inside the window. A
proposal: the decisions it needs are listed at the end and get their ADRs once taken.

## The shape

One catalogue of tools over `core-engine`, put in front of a model three ways. The model
orchestrates; every geometric choice — a pose, a wall, a hole, a tip — is the engine's; a
strict check decides when the work is done.

```
 Claude Desktop, Claude Code, Cursor ── stdio ──> encrust-mcp ─────────┐
 ChatGPT, claude.ai, mobile ── HTTPS ──> encrust.app Worker            │
                                           └──> Container: encrust-mcp --http
 the window's agent panel ── in process ───────────────────────────────┤
                                                                       v
                                 agent-tools: the catalogue, its schemas, the plate state
                                                                       v
                                       core-engine, core-plate, core-volume, core-supports …
```

`agent-tools` holds what every transport shares, so a tool behaves the same in every client
and the window (rule 9 of `architecture.md` extended to the model as a third front end).
`encrust-mcp` is the MCP server over it: stdio for a desktop client, Streamable HTTP for the
site.

## The tools

| Tool | Does | Engine | Kind |
|---|---|---|---|
| `open_model` | reads a file or an upload, welds, diagnoses, repairs when asked | `core-mesh-io`, `mend` | creates a plate |
| `list_profiles` | printers, the user's resins, support profiles | `Catalogue` | read |
| `orient` | the search's finalists with their readings; applies one | `finalists`, `orient` | edits |
| `hollow` | the wall, infill, open floor; the pockets left trapped | `hollow`, `TrapScan` | edits |
| `suggest_drains` | ranked hole positions that empty every pocket | new, see below | read |
| `drill` | holes at given points, plugs | `drill`, `plugs` | edits |
| `support` | a profile or auto, the cavity too; contacts left without one | `stand_under` | edits |
| `check` | every finding, located: kind, layer, plate point, size | `Measured`, `TrapScan` | read |
| `render` | the plate from a view, findings marked, as an image | `core-thumbnail` | read |
| `estimate` | time, resin, weight, cost | `print_out` | read |
| `write` | the printer file; refused unless `check` is clean or the user accepts it | `core-pipeline` | writes a file |
| `send` | the file to a printer on the user's network | `printer-link` | outward, confirmed by a person |
| `prepare` | the whole chain with the engine's defaults, no model involved | new, see below | edits |

Each tool carries MCP annotations (`readOnlyHint`, `destructiveHint`) so a client asks
before `write` and `send`. A result is JSON for the model plus, from `render`, an image; a
finding is always located, so the next call can act on it.

### State

MCP 2026-07-28 is stateless: no session, any request on any instance. A plate is therefore
named in every call by a handle, and the handle is a `.encrust` project revision — it
already holds the shells and the support trees as they stand (ADR 0191). Each editing tool
returns a new revision, which gives undo and lets any instance serve the next call; a
process caches the opened plate by revision so a chain of calls does not reopen it. Locally
a revision is a file beside the model; on the site it is an object in R2.

## Engine work first

These help the command line and the window as much as a model, and nothing above is worth
building without them.

1. **Located findings.** `--json` gives counts: `trapped_pockets`, `islands`,
   `suction_cups` (`encrust-cli/src/batch/report.rs`). The engine knows where each is —
   `Trapped::at` and `bounds`, a `Risk`'s layer and piece, every contact a support could not
   stand under. The report lists them, additively, and `docs/cli.md` documents the fields.
2. **Suggested drains.** For each trapped pocket, the lowest points of the cavity in the
   print's pose, on faces looking down or out of sight, clear of supports and of each other,
   each proven by drilling it and scanning again. A model picking `--drain-at X,Y,Z` by
   itself is the weakest link of the chain; this makes it a choice among proven candidates.
   The command line gets it as `--drain auto`.
3. **Views with markers.** `core-thumbnail` renders the plate from four sides and from
   below with each finding marked, for a model that reads images, and for `encrust render`.
4. **An autopilot.** `prepare`: repair, `--orient-goal auto`, hollow where the wall saves
   resin, suggested drains, `--supports auto --support-cavity`, check, write under
   `--strict`. It is the baseline every model is measured against, and what a tool call
   runs when the model has nothing to add.

## The transports

### On the user's computer

`encrust-mcp` as a binary over stdio, packed as an `.mcpb` bundle per platform
(`server.type = "binary"`) for a double-click install in Claude Desktop; Claude Code,
Cursor, Codex and LM Studio take the same binary from their config. The files are the
user's own paths, the compute is free, and it is the only transport that reaches a printer
on the user's network, so `send` exists here alone. ChatGPT does not take stdio servers.

### On encrust.app

A Worker cannot run the engine: an isolate has 128 MB, and a hollowed one-million-face
figure took 257 MB of wasm memory (`web-build.md`, measured 2026-10-03). The site splits
the work:

- **The Worker** is the MCP endpoint: OAuth for the user (client ID metadata documents,
  dynamic registration for older clients), uploads and outputs in R2 behind signed URLs,
  limits per user, and the routing of each heavy call to a container.
- **A Container** runs `encrust-mcp --http`, native and on every core: `standard-4` has
  4 vCPU and 12 GiB. A job of 30 s on 4 cores and 12 GiB costs about half a cent at the
  published rates; a cold start adds seconds, and a container sleeps when idle.
- **Files.** A model does not fit in a tool's arguments. ChatGPT hands the widget a file
  (`uploadFile`) and the tool a reference; elsewhere the widget uploads to a signed R2
  URL. The result is a download link; the printer is reached by the local server or the
  window.

### Inside the chat, on the user's machine

MCP Apps (SEP-1865, stable since 2026-01-26, shown by Claude and ChatGPT) lets a tool
return a `ui://` page that the client renders in a sandboxed frame. That frame can show the
plate in 3D, findings marked, and could run `web-engine` itself: one thread, so it needs no
cross-origin isolation, and the compute costs the site nothing. Unknown until tried: whether
each client's frame policy allows `'wasm-unsafe-eval'`, how much memory a frame gets, and
`web-engine` slicing only a finished project today. A spike decides before anything rests
on it.

### In the window

An agent panel in `encrust-app` runs the same catalogue in process on the plate that is
open, so each step lands in the viewport and on the undo stack, and runs on the job
threads the window already uses, never on the page's thread in a browser. The provider is
the user's: the Anthropic Messages API with tool use, or any OpenAI-compatible endpoint,
which covers local models (Ollama, LM Studio). A key lives in the system's keychain; the
browser build reaches a provider through the site's Worker so no key sits in the page.

## What keeps it safe

- No exposure is invented: a resin is the user's (ADR 0196), and `write` without one is
  refused as `slice` refuses it now.
- `write` passes only a clean `check` unless the user accepts what it names; `send` waits
  for a person's confirmation in every client.
- The agent loop and every retry have a stated bound; each tool can be cancelled.
- A foreign file is read as `hostile-files.md` says; on the site each job runs in its own
  container with size and time limits.
- The served program offers its source, as the web build does for the AGPL.

## How quality is measured

An `agent-lab` beside `orient-lab` and `support-lab`, on their corpora: each model run by
the autopilot and by each model and client under test, scored on what the labs already
read — the strict status, islands, trapped resin, tips on faces looking up, resin, print
time — plus tool calls and tokens, against a kept baseline. A model's path is offered where
it matches or beats the autopilot, and the server's `instructions` and prompts carry the
procedure it is measured with.

## Steps

| Step | What | Where | After |
|---|---|---|---|
| 1 | Located findings in the report | `core-analysis`, `core-supports`, `encrust-cli` | — |
| 2 | Suggested drains, `--drain auto` | `core-volume`, `core-supports`, `encrust-cli` | — |
| 3 | Views with markers, `encrust render` | `core-thumbnail`, `encrust-cli` | — |
| 4 | The autopilot, `encrust prepare` | `core-engine`, `encrust-cli` | 1, 2 |
| 5 | The catalogue and plate revisions | `agent-tools` | 1 |
| 6 | The stdio server and its bundle | `encrust-mcp` | 5 |
| 7 | The bench | `agent-lab` in `encrust-cli` | 4, 6 |
| 8 | The site: Worker, Container, OAuth, R2 | `encrust-mcp --http`, the site repository | 6 |
| 9 | The chat widget; the client-side compute spike | `encrust-mcp`, `web-engine` | 6 |
| 10 | The window's agent panel | `encrust-app` | 5 |

Steps 1, 2 and 3 touch separate crates and run in parallel; 8, 9 and 10 likewise once 6
stands.

## How the work is run

- One integrator session owns this page, the interfaces between steps, the ADRs and the
  merges.
- A writer per step, in its own git worktree, briefed with the rules, the interface it
  implements and the tests that accept it. Steps split along crate boundaries, so no two
  writers touch one file.
- A reviewer per step that is not its writer, holding the diff to `review.md`; its findings
  go back to the writer, and the integrator settles what two disagree on.
- Each worktree builds into its own `target/`, which costs a full build of disk and cores;
  two writers at a time is the ceiling here, and each keeps to one cargo at a time.

## Open decisions

1. **Two crates and their dependencies:** `agent-tools` (on `core-engine`, `serde`, and
   `schemars` for the schemas) and the binary `encrust-mcp` (`rmcp`, `tokio`). Recommended:
   yes, a binary of its own keeps `tokio` out of the command line and the cores.
2. **The report:** located findings added to schema 1, every existing field kept.
   Recommended: additive, no new schema number.
3. **The plate handle:** a `.encrust` revision. Recommended: yes; it exists and the window
   opens it.
4. **Hosting:** Cloudflare Containers beside the site, on the Workers Paid plan, or a host
   outside Cloudflare. Recommended: Containers, one account and one bill.
5. **Compute in the chat's frame:** a spike only, until it runs in both clients.
6. **The window's provider:** the user's key, Anthropic and OpenAI-compatible.
   Recommended: yes, local models included.

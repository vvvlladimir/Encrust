# AI integration

How a language model takes a model from its file to a checked printer file through Encrust,
on the user's computer: driven from a chat client over MCP, or from inside the window.
Nothing runs on encrust.app. The server is ADR 0395; the rest is the plan.

## The shape

One catalogue of tools over the engine, put in front of a model three ways. The model
orchestrates; every geometric choice — a pose, a wall, a hole, a tip — is the engine's; a
strict check decides when the work is done.

```
 Claude Desktop, Claude Code, Codex, Cursor ── stdio ──> encrust-mcp ──┐
                                        └── or into the open window ───┤
 the window's agent panel (step 9) ── in process ───────────────────────┤
                                                                       v
                         encrust --json, a child per call (the panel: the scene)
                                                                       v
                                       core-engine, core-plate, core-volume, core-supports …
```

`encrust-mcp` runs the command line for every tool, so a tool can do what `encrust` does and
nothing else (rule 9 of `architecture.md` extended to the model as a third front end). The
catalogue moves into a crate of its own when the window's panel needs it too.

## The tools

| Tool | Does | Command line | Kind |
|---|---|---|---|
| `list_profiles` | printers, the user's resins, support profiles | `profiles list` | read |
| `inspect_model` | a file's size, shells, defects, what a repair would do | `inspect` | read |
| `start_job` | a job for a model, printer and resin, checked as it stands | `estimate` | creates a job |
| `set_pose` | the orientation search or a turn; clears the drains | `--orient`, `--rotate` | edits |
| `set_hollow` | the wall, infill, open floor | `--hollow` and its flags | edits |
| `set_drains` | holes at plate points, plugs | `--drain`, `--drain-at` | edits |
| `set_supports` | a profile or auto, the cavity too | `--supports` | edits |
| `check` | a revision as it would print | `estimate --check-drainage` | read |
| `revert` | an earlier revision made the latest again | — | edits |
| `write_file` | the printer file; refused while the check names defects | `slice --strict` | writes a file |
| `render` | pictures from each side, the check's findings marked | `render --mark` | read |
| `send_to_printer` | the file to a printer on the user's network | `printer send` | outward |

Still to come: `prepare`, the autopilot of step 4. `set_drains` with `auto` drills the drains
step 2 proves. An
edit is kept only if the command line took it; every finding is located (ADR 0394), so the
next call can act on it.

### State

MCP 2026-07-28 is stateless: no session, any request on any instance. A call therefore
names its job, a directory in the user's cache of numbered revisions, each a TOML recipe —
the model, printer, resin and each step's settings — that becomes `encrust` flags. An
editing tool writes the next revision and checks it; `revert` copies an old one forward.
The cost: each check stages the model again from its file (ADR 0395).

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
   The command line gets it as `--drain-at auto` (ADR 0396; `--drain` is the diameter).
3. **Views with markers.** `core-thumbnail` renders the plate from four sides and from
   below with each finding marked, for a model that reads images, and for `encrust render`.
4. **An autopilot.** `prepare`: repair, `--orient-goal auto`, hollow where the wall saves
   resin, suggested drains, `--supports auto --support-cavity`, check, write under
   `--strict`. It is the baseline every model is measured against, and what a tool call
   runs when the model has nothing to add.

## The transports

### From a chat client

`encrust-mcp` as a binary over stdio, packed as an `.mcpb` bundle per platform
(`server.type = "binary"`) for a double-click install in Claude Desktop; Claude Code,
Codex, Cursor and LM Studio take the same binary from their config. The files are the
user's own paths and the printer is on the user's network. The client runs the agent loop
on the user's own plan, so a subscription to Claude or ChatGPT is all it takes: this is the
way a subscription reaches Encrust (see Providers).

When the window is open, `encrust-mcp` hands each call to it over a local socket instead of
running it alone, so every step lands in the viewport and on the undo stack as the client
makes it. With no window it runs the engine itself.

An MCP Apps page (SEP-1865, `ui://`, shown by Claude and ChatGPT) can show the plate in 3D
with its findings beside the chat; the local server serves it. Optional, after the tools.

### In the window

An agent panel in `encrust-app` runs the same catalogue in process on the plate that is
open, so each step lands in the viewport and on the undo stack, and runs on the job
threads the window already uses, never on the page's thread in a browser. Its provider is one of those below.

### Providers

| Provider | Needs | In the window | Over MCP |
|---|---|---|---|
| A Claude subscription | Claude Desktop or Claude Code | no: Anthropic lets no other product sign in with it | yes |
| A ChatGPT subscription | Codex, or ChatGPT's desktop app | only through OpenAI's own "Sign in with ChatGPT" for apps, once its terms are read | yes |
| An API key | Anthropic, OpenAI or any OpenAI-compatible service | yes, the key in the system's keychain | yes |
| A local model | Ollama or LM Studio | yes, no key and no account | yes |

The window never borrows a client's login token, nor drives an installed Claude Code as
its engine: neither is a route its maker offers. A small local model calls tools less
reliably, which the autopilot covers: it asks the model only what the engine cannot settle.

## What keeps it safe

- No exposure is invented: a resin is the user's (ADR 0196), and `write` without one is
  refused as `slice` refuses it now.
- `write` passes only a clean `check` unless the user accepts what it names; `send` waits
  for a person's confirmation in every client.
- The agent loop and every retry have a stated bound; each tool can be cancelled.
- A foreign file is read as `hostile-files.md` says.

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
| 1 | Located findings in the report | `core-raster`, `core-engine`, `encrust-cli` (done, ADR 0394) | — |
| 2 | Suggested drains, `--drain-at auto` | `core-volume`, `core-engine`, `encrust-cli` (done, ADR 0396; the window's Auto to come) | — |
| 3 | Views with markers, `encrust render` | `core-thumbnail`, `encrust-cli` (done) | — |
| 4 | The autopilot, `encrust prepare` | `core-engine`, `encrust-cli` | 1, 2 |
| 5 | The stdio server, its jobs and recipes | `encrust-mcp` (done, ADR 0395) | — |
| 6 | Its tools for steps 1–4, and the `.mcpb` bundle | `encrust-mcp` (1–3 and the bundle done) | 1–5 |
| 7 | The bench | `agent-lab` in `encrust-cli` | 4, 6 |
| 8 | `encrust-mcp` driving the open window | `encrust-mcp`, `encrust-app` | 6 |
| 9 | The window's agent panel and its providers; the catalogue moves down | `encrust-app` | 6 |
| 10 | The MCP Apps page | `encrust-mcp` | 6 |

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

1. **The window as the server's engine:** a local socket the open window listens on, after
   the stdio server works alone.
2. **The window's providers:** an API key and local models first; "Sign in with ChatGPT"
   once OpenAI's terms for apps are confirmed.

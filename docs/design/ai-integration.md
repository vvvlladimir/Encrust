# AI integration

How a language model takes a model from its file to a checked printer file through Encrust,
with no window open: from Claude Desktop through a local extension, from a web chat that
reaches only remote servers through a connector whose work runs in the user's browser, or from the window's own panel. The engine
computes on the user's machine in every case; nothing that reads a model runs on encrust.app.
The server is ADR 0455; the rest is the plan.

## What the model is for

A recipe (`recipes.md`) already prepares a model in one press, the same way every time, for
free. A language model is worth its cost only where it knows what the engine does not:

| The model brings | The engine turns it into | Without the model |
|---|---|---|
| **What the model is** — a face, a gear's bore, a hidden base — read off `render` | painted faces (`Shown`, ADR 0425 of main) and places kept clear, which orientation and Auto honour | `--clean-surface` guesses what shows from the shape alone |
| **What the user wants** — display, tabletop, a part that must fit; resin or time | the recipe and its variables to run (`prepare`, `miniatures`, one of theirs) | `prepare` for everything |
| **The resin they have** — a file that printed well, another slicer's export, an exposure test's photo | a resin profile from `profiles import` (ADR 0293, 0295), or a setting chosen on the test print | the window's resin editor |
| **What went wrong** — the photo of a failed print, read against the job that wrote it | the step to change and how, as a new revision checked like any other | nothing: no recipe sees a print |
| **A way of working in words** — "busts like this, every time" | a recipe file `core-recipe` validates and the window runs with no model | the recipe editor |

What stays the engine's: every number with a unit (a wall, a tip, an exposure), every
geometric choice, and whether a file is fit to print. A model never invents an exposure, and
a tool that only does what a recipe already does is not added.

## The shape

One catalogue of tools over the engine, put in front of a model three ways. The model
orchestrates; every geometric choice — a pose, a wall, a hole, a tip — is the engine's; a
strict check decides when the work is done.

```
 Claude Desktop, Claude Code, an MCP client ── stdio ──> encrust-mcp ──┐
                                        └── or into the open window ───┤
 the window's agent panel (step 9) ── in process ───────────────────────┤
                                                                       v
                         encrust --json, a child per call (the panel: the scene)
                                                                       v
                                       core-engine, core-plate, core-volume, core-supports …

 a web chat ── HTTPS ──> the connector (routes calls, holds nothing) ──> its page in the chat
                                                                       v
                                         web-engine in the user's browser: the same cores
```

`encrust-mcp` runs the command line for every tool, so a tool can do what `encrust` does and
nothing else (rule 9 of `architecture.md` extended to the model as a third front end). The
connector's page calls `web-engine`, which makes the same core calls. The catalogue moves into
a crate of its own when the window's panel or the connector needs it too.

## The tools

| Tool | Does | Command line | Kind |
|---|---|---|---|
| `list_profiles` | printers, the user's resins, support profiles | `profiles list` | read |
| `inspect_model` | a file's size, shells, defects, what a repair would do | `inspect` | read |
| `start_job` | a job for a model, on the printer and resin named or the standard ones (ADR 0475), checked as it stands | `estimate` | creates a job |
| `set_pose` | the orientation search, one of its ways or a turn; places kept clear; clears the drains | `--orient`, `--orient-pick`, `--orient-keep-clear`, `--rotate` | edits |
| `compare_poses` | the search's best ways, each with what it prints and drawn | `estimate --orient-choices`, `render --orient-pick` | read |
| `set_hollow` | the wall, infill, open floor | `--hollow` and its flags | edits |
| `set_drains` | holes at plate points or by the search, channels, plugs | `--drain`, `--drain-at` | edits |
| `set_supports` | a profile or auto, the cavity too | `--supports` | edits |
| `check` | a revision as it would print | `estimate --check-drainage` | read |
| `revert` | an earlier revision made the latest again | — | edits |
| `write_file` | the printer file, on the printer and resin the user names; refused while the check names defects | `slice --strict` | writes a file |
| `render` | pictures from each side, the check's findings marked | `render --mark` | read |
| `send_to_printer` | the file to a printer on the user's network | `printer send` | outward |
| `show_in_window` | a revision on the open window's plate, the window started if closed (ADR 0465) | — | edits the window |
| `set_supports` `shown_at_mm` (step 13) | the side meant to be seen, painted round plate points | `--shown-at` | edits |
| `list_recipes`, `use_recipe` (step 14) | the recipes with their steps; one for the job, by the user's purpose | `recipes list`, `recipes show`, `--recipe` | read, edits |
| `import_resin` (step 15) | a resin from a sliced file or another slicer's export | `profiles import` | creates a profile |
| `create_resin` | a resin from its maker's numbers, held to the printer's limits | `profiles new-resin` | creates a profile |
| `write_test_print` (step 15) | a test print, the exposure check among them | `test-print` | writes a file |
| `report_print` (step 16) | what a print of a job's file did, as named symptoms, read against its check | `diagnose` | edits |
| `save_recipe` (step 17) | a recipe from the job's revisions or from words, validated | `recipes save` | creates a profile |

`start_job` runs the autopilot (`--prepare`) only when the client says `prepare: true`, as
it does when the user asked to prepare the model for print, and each `set_*` tool overrides
one of its steps; to open or cut a model, it says false. An edit is kept only if the command line took it; every finding
is located (ADR 0400), so the next call can act on it.

### Better than the autopilot

A client beats the autopilot only by what it knows and the engine does not: what the model
is. So it chooses, and the engine proves (ADR 0458). `compare_poses` lays the search's
finalists side by side, each printed on a trial and drawn; the client picks one for a reason
it can state, and names the places no support should touch — a face, a lip — which the proof
ranks first. Each check answers with `compared`, set against the revision before and the
autopilot's own, so an edit that falls behind is reverted unless the user asked for it.

### After the print

The loop a recipe cannot close. A job notes the revision it last wrote, so `report_print`
finds the one printed. The client reads the user's words or photo and names symptoms from a
fixed list — the part off the plate, a layer split, a hole or a crater, a support pulled
from the model, a mark on a face, a crack in a hollow print. `encrust diagnose` reads each,
against that revision's report, to the step that likely caused it and the change to try
([print-failures.md](print-failures.md), ADR 0463); the change is an edit like any other,
checked and compared. The map is data the command line reads, not the model's memory, so
it is tested and improved by prints rather than by prompts.

### State

MCP 2026-07-28 is stateless: no session, any request on any instance. A call therefore
names its job, a directory in the user's cache of numbered revisions, each a TOML file —
the model, printer, resin and each step's settings — that becomes `encrust` flags, and
beside it the `core-recipe` its check ran. With the window followed, the window owns the job's plate,
a plate of its own (ADR 0466, 0477): a revision whose every change is an edit is checked on it and kept
as that plate and its edits (`N.encrust`, `N.edits.json`), and handed to the window as edits,
so what the user did there is in the next check; any other revision the window runs by its
recipe (ADR 0459). An editing tool writes the next revision and checks it; `revert` copies an old one forward;
`report_print` appends what a print did. The cost: each check stages the model again from
its file (ADR 0455). The connector keeps the same revisions in the page's own state, never
on the server.

## What main brought

`main` built steps 1, 2 and 4 under its own records (0400, 0401, 0402) and went further; the
merge kept main's, renumbered the branch's own after 0454 and left out what no longer fits:

| Here | In main | Now |
|---|---|---|
| `--prepare` fills the flags left out | 0402, superseded by recipes (0403) | `--prepare` is `--recipe prepare`; `start_job` runs it |
| a job's TOML "recipe" | `core-recipe`'s recipe | each revision keeps the `core-recipe` its check ran (ADR 0459) |
| `--keep-solid X,Y,Z` | `--keep-solid` with blockers fitted to the model (0396) | main's; the branch's flag dropped |
| `keep_clear_at_mm` points | painted `Shown` faces (0410, 0425), window only | both stand: kept clear for the search, painted for the supports (ADR 0460) |
| the window replaying each revision | Prepare for print rebuilt on recipes, then one plate edited by one apply | step 8 again: the window takes a revision as edits of the plate it owns, and runs by its recipe only what no edit carries yet (ADR 0459, 0466) |

Main also brings what the tools gain without new code: Check's issues located and fixed by
the `fix-issues` recipe, cups vented by their thinnest wall, necks held by their stress, one
cavity through parts at the thinnest wall asked for, Auto sized by what each tip holds, and
release builds for ARM64 Windows and Linux (0454), which the extension ships for too.

## The transports

### Claude Desktop: an extension in one download

`encrust-mcp` as a binary over stdio, packed with `encrust` as an `.mcpb` bundle per
platform (`server.type = "binary"`, `cargo xtask mcpb`). Double-clicking it installs it in
Claude Desktop; Claude Code and other MCP clients take the same binary from their
config. The files are the user's own paths and the printer is on the user's network.

- **Built by the release.** `release.yml` packs a bundle for each of its six targets after
  the build and attaches `encrust-<os>-<arch>.mcpb` to the release, signed with `mcpb sign`
  where the certificate is set (ADR 0461).
- **Downloaded from a button.** encrust.app's download page offers the bundle for the
  visitor's system beside the installer; the window's File → Use with Claude Desktop
  downloads the one for its own version.
- **Updated by a new download.** Anthropic's directory no longer takes `.mcpb` listings, so
  nothing updates it in place; the bundle reports its version in `instructions`.
- **No window needed.** Resins come from `import_resin` and the exposure test (step 15).
  When the window is open, each revision's recipe runs on its plate (ADR 0459, `window-link.md`).

Claude on the web and on a phone runs no local program and gives an MCP App page no file
from the chat; it is served once one of those changes.

### A web chat: a connector whose page computes

A web chat that runs no local program reaches only a remote server. The connector is that server kept thin: it lists the
tools and serves one MCP Apps page (SEP-1865), and does no geometry.

- **The file.** `start_job` declares its model as a file parameter in the host's metadata, so the file the user
  attaches in the chat arrives as a short-lived `download_url` and a `file_id`. The server
  hands the URL to the page, which fetches the bytes and keeps them in the browser under the
  `file_id`.
- **The work.** The page runs `web-engine` (ADR 0177): repair, the recipe's steps, the check
  and the pictures, each the same core call the command line makes. `web-engine` gains these
  entry points; today it slices a project only.
- **The answer.** Each tool's result carries the revision; the page computes it and returns
  the report and the pictures to the model, which reads them before its next call.
- **The file out.** The page offers the printer file as a download. Sending to a printer is
  not offered: the browser cannot reach the user's network.

Step 18 is a spike (commit 602ae603, its throwaway code since removed) that settles, in such a chat on the web and its desktop app, with a 20 MB STL:
the page can fetch the `download_url`; a prepare of a figure finishes within a minute on a
laptop with no cross-origin isolation; the report reaches the model in the same turn; the
file downloads. A failed answer stops the connector, and that chat's users go through a local MCP client,
which runs the local server today.

### In the window

An agent panel in `encrust-app` runs the same catalogue in process on the plate that is
open, so each step lands in the viewport and on the undo stack, and runs on the job
threads the window already uses, never on the page's thread in a browser. Its provider is one of those below.

### Providers

| Provider | Needs | In the window | Over MCP |
|---|---|---|---|
| A Claude subscription | Claude Desktop or Claude Code | no: Anthropic lets no other product sign in with it | yes, the extension |
| Another chat subscription | its web chat for the connector, or its local MCP client | only through a sign-in its maker offers to apps, once its terms are read | the connector, or its client locally |
| An API key | Anthropic or any compatible service | yes, the key in the system's keychain | yes |
| A local model | a local model server | yes, no key and no account | yes |

The window never borrows a client's login token, nor drives an installed Claude Code as
its engine: neither is a route its maker offers. A small local model calls tools less
reliably, which the autopilot covers: it asks the model only what the engine cannot settle.

## What keeps it safe

- No exposure is invented: a resin is the user's (ADR 0196), imported from what they printed
  or chosen on their own test print, and `write` without one is refused as `slice` refuses it.
- `write` passes only a clean `check` unless the user accepts what it names; `send` waits
  for a person's confirmation in every client.
- A symptom changes a step only through an edit that is checked and compared like any other.
- The agent loop and every retry have a stated bound; each tool can be cancelled.
- A foreign file is read as `hostile-files.md` says; the connector's server never holds one.

## How quality is measured

`agent-lab` (ADR 0456; `docs/cli.md`) prepares each model by the autopilot and by each
client under test, run as its own command against `encrust-mcp`, and scores both on the
check's report — status, islands, suction cups, trapped pockets, contacts with no room,
resin, print time — with the client's tool calls from the server's trace and its tokens from
its own output, against a kept baseline. `--judge` draws both preparations and asks another
model which prints better, so marks on a visible face count, and no model grades its own
work. A model's path is offered where it matches or beats the autopilot, and the server's
`instructions` carry the procedure it is measured with.

Each row of the first table gets a set of its own, where the autopilot lacks what the model
brings:

- **Meaning:** figures whose shown faces a person painted; scored by contacts on those faces.
- **Intent:** models with a stated purpose; scored by the recipe chosen against a person's.
- **After the print:** photos of failed prints whose cause is known; scored by the step and
  change named against the known one.
- **Recipes from words:** descriptions with a target set; the recipe must validate and match
  or beat `prepare` on that set.

A kept handful of each set is printed, and what came off the plate is recorded beside the
score; a score that the prints contradict is fixed before any prompt is tuned.

## Steps

| Step | What | Where | After |
|---|---|---|---|
| 1–4 | Located findings, drains proposed, views with markers, the autopilot | engine and command line (done; main's 0400–0403) | — |
| 5–7 | The stdio server and its jobs, its tools and `.mcpb`, the bench | `encrust-mcp`, `encrust-cli` (done, ADR 0455, 0456) | — |
| 11 | Poses offered and chosen, places kept clear, each check compared, a judge; supports by hand | (done, ADR 0457, 0458) | 6, 7 |
| 12 | Merge main | (done) | — |
| 8 | The open window owns the plate: a revision reaches it as edits, the rest by its recipe | `core-edit`, `core-document`, `encrust-cli`, `encrust-mcp`, `encrust-app` (done, ADR 0459, 0466, 0468) | 12 |
| 13 | The side meant to be seen painted from the command line, `--shown-at X,Y,Z[:R]` | `core-engine`, `encrust-cli`, `encrust-mcp` (done, ADR 0460) | 12 |
| 14 | Recipes chosen by purpose: `list_recipes`, `use_recipe`, the instructions ask what the print is for | `encrust-mcp` (done, ADR 0460) | 12 |
| 15 | Resin with no window: `import_resin`; `encrust test-print`, the window's test prints moved into `core-samples` | `core-samples`, `encrust-cli`, `encrust-mcp` (done, ADR 0460); a resin from a label's numbers by `create_resin` (done, ADR 0464) | 12 |
| 16 | After the print: `report_print`, `print-failures.md` and the command line reading it | `encrust-cli`, `encrust-mcp` (done, ADR 0463) | 12 |
| 17 | `save_recipe`: a job's revisions or a description as a validated recipe | `encrust-cli`, `encrust-mcp` (done, ADR 0462) | 14 |
| 18 | The connector's spike: file in, a prepare in the page, report to the model, file out | the spike, removed after 602ae603 (built; a prepare in an unisolated page measured at 20 s for a 175k-face figure; the rest waits on a run in a web chat) | 12 |
| 19 | `web-engine` prepares, checks and draws; the connector and its page | `web-engine`, a new crate | 18 |
| 20 | The extension shipped: bundles in `release.yml`, signed where the certificate is set, File → Use with Claude Desktop | `.github`, `xtask`, `encrust-app` (done, ADR 0461); the site's button is the site's | 12, 15 |
| 21 | The bench's sets per use, and their prints | `agent-lab` | 13–17 |
| 9 | The window's agent panel and its providers; the catalogue moves down | `encrust-app` | 12 |
| 10 | The MCP Apps page in Claude Desktop, the connector's page served locally | `encrust-mcp` | 19 |

Order: 19 once the spike's live run answers; 21 grows with each; 9 and 10 last.

## How the work is run

- One integrator session owns this page, the interfaces between steps, the ADRs and the
  merges.
- A writer per step, in its own git worktree, briefed with the rules, the interface it
  implements and the tests that accept it. Steps split along crate boundaries, so no two
  writers touch one file.
- A reviewer per step that is not its writer, holding the diff to `review.md`; its findings
  go back to the writer, and the integrator settles what two disagree on.
- Each worktree builds into its own `target/`, which costs a full build of disk and cores;
  two writers at a time is the ceiling here, and each keeps to one cargo at a time. A
  `target/` shared between worktrees reuses a build script's output across them: one that
  embeds files by path (`printer-profiles`) then ships another checkout's files, or none.

## Open decisions

1. **The window's providers:** an API key and local models first; another maker's sign-in
   once its terms for apps are confirmed.
2. **The connector's server** (before step 19, an ADR): where it is hosted on a free tier,
   and in what language; it sees a file's URL, never its bytes.
3. **Photos** (settled by ADR 0463): the client names symptoms from the fixed list, and no
   image reaches the engine.

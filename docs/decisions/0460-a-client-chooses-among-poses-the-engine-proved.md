# 0460. A client chooses among the poses the engine proved

- **Status:** Accepted
- **Date:** 2026-11-03

## Context

A chat client could only rerun the autopilot's search or turn a model by angles it guessed, so
it could match the autopilot, never beat it. What it knows that the engine does not is what the
model is — which face shows — and it can look at pictures. Language models in a printing loop
(arXiv 2603.22118) are reliable choosing among options an optimiser proved and unreliable
writing geometry. The search already prints its best few poses (ADR 0290). A later edit reran
the search, so a new lift could turn the model unasked; a turn carried the old pose's contacts
(ADR 0439), leaving islands under the new one.

## Decision

- **The finalists are offered:** `--orient-choices N` reports the first N poses with what each
  trial printed; `--orient-pick N` stands the Nth. The server's `compare_poses` draws each, and
  `set_pose` takes `pick`.
- **Places kept clear:** `--orient-keep-clear X,Y,Z` in plate mm as the client saw them; each
  trial counts contacts near them and the sound poses touching fewest rank first. A pose that
  loses the print is never raised above a sound one.
- **Every check is set against the revision before and the autopilot** (`estimate --against`,
  `encrust_cli::score`); the server answers each edit with `compared`. `agent-lab` draws both
  preparations, and `--judge` asks another model which prints better.
- **A searched pose is kept as its turn** (`stood_deg`) once checked; later revisions stand by
  `--rotate` and search no more. A new `set_pose` asks anew.
- **A new pose stands the supports anew**: `Edit::Supports` takes `anew`, which clears points
  and frozen trees, painted regions kept; the server sends it after every change of pose, and
  `set_pose` clears points given by hand on the old pose.

## Consequences

A client can keep a face clean or take the second pose for a reason it states, while the engine
proves every option and the check decides. `compare_poses` costs a render a choice. A turn kept
is not proved again on a new machine. Supports grown by hand in the window are lost on a
client's turn. Reopen if clients pick worse than the first choice on the bench.

## Alternatives considered

### A penalty in the search's own score

The search ranks thousands of directions before any contact is known; ranking the proof's
finalists keeps it as it is, but a pose dropped early is never offered.

### Keep the contacts that still face down after a turn

Keeps more of what stood, but contacts chosen for the old pose are not where the planner puts
them for the new one.

### The option that won, and what it costs

A field the client never sees, a pose not rechecked when the machine changes, and one more
server decision on when to stand supports anew.

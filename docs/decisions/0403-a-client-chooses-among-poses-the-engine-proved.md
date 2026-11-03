# 0403. A client chooses among the poses the engine proved, and is held to its own last step

- **Status:** Proposed
- **Date:** 2026-11-03

## Context

A chat client preparing a print over `encrust-mcp` (ADR 0397) could only rerun the search the
autopilot runs, or turn the model by angles it guessed: it could match the autopilot, never
beat it. What a client knows that the engine does not is what the model is — which face is a
face, which lip shows — and it can look at pictures. Research on language models in a
printing loop (arXiv 2603.22118) finds them reliable as a choice among options an optimiser
proved, with structured readings, and unreliable writing geometry outright; CAD agents
(CADSmith, AgentsCAD) check a model's work with a judge of its own on rendered views. The
search already prints its best few poses to rank them (ADR 0290); each edit is checked, but
nothing says whether it made the job better than the revision before or than the autopilot,
and `agent-lab` (ADR 0400) scores numbers only.

## Decision

**The search's finalists are offered, not only the first.** `--orient-choices N` reports the
first N ranked ways to stand a model, each with what its trial printed — contacts, those on
faces looking up, hanging contacts, support and model resin, layers, the hardest pull, islands,
cups, overflow. `--orient-pick N` stands the model the Nth way. `encrust-mcp`'s `compare_poses`
lists them with a picture each, and `set_pose` takes `pick`.

**Places can be kept clear of supports.** `--orient-keep-clear X,Y,Z`, in plate mm as the model
stood turned by `--orient-keep-clear-turn` and centred, is taken back into the model's frame
(`core_engine::KeepClear`); every trial counts its contacts within
`--orient-keep-clear-radius` of a point, and the ranking puts the sound poses touching fewest
first, its own order kept between equals. A pose that loses the print — a cup, a tearing pull,
an overflow — is never raised above a sound one. The server fills the turn and takes off the
lift from the revision the client looked at, so the client gives points as it sees them.

**Every check is set against the revision before and the autopilot.** `estimate --against
NAME=FILE` judges the run against kept reports by `agent-lab`'s rule, now
`encrust_cli::score`; the server keeps the autopilot's check of each job beside it and answers
every edit with `compared`. Its instructions tell the client to revert an edit that falls
behind unless the user asked for it.

**`agent-lab` draws both preparations and asks a judge of its own.** `--pictures` renders each
from the front and below from the flags the server now keeps beside each revision
(`N.args.json`); `--judge COMMAND` shows another model both, with their scores, and keeps its
verdict beside the numbers.

The window replays `pick` and the points kept clear through the same engine calls.

## Consequences

A client can now do what the autopilot cannot — keep a face clean, take the second-best pose
for a reason it states — while the engine still proves every option and the check still
decides. `compare_poses` costs one render a choice on top of a search; its pictures are
small. Keep-clear points depend on the turn they were seen in, so the recipe keeps that turn;
a model changed in shape after they were given (a relief, a cut) can move them a little, which
the radius absorbs. The judge's verdict is an opinion of another model, kept but not counted
in the standing. Reopen if clients pick worse than the first choice on the bench, or if a
search should optimise keep-clear in its own score rather than only in the proof's ranking.

## Alternatives considered

### Let the client turn the model by angles alone

It already could, and the bench shows it only loses to the search: the client cannot weigh
supports and peel as a trial can.

### Points in the model file's own frame

Exact, and independent of the pose, but a client sees the plate in renders and findings, and
would have to work the file's frame out from them; the server knows the turn and the lift.

### A penalty in the search's own score

The search ranks thousands of directions before any is printed, with no contacts to count; the
proof prints the finalists, where contacts are known. Ranking there keeps the search as it is,
and costs nothing when no point is given — but a pose the search dropped early is never offered.

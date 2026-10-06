# 0191. A project holds the plate as it stands, and opening it builds nothing

- **Status:** Accepted
- **Date:** 2026-10-06

## Context

ADR 0097 keeps only what the user asked for in a `.encrust` file, and ADR 0178 added the
wall each cavity was built to so that a plate could be hollowed again on opening. Opening
a project therefore ran the Hollow tool and the automatic support run a second time, from
the manifest rather than from what the user had in front of them.

That makes the opened plate a different plate from the saved one. A cavity's lattice is
coarsened to fit a memory budget, which is the machine's, not the file's, so the same
project hollows differently on another computer or in a browser. The support run decides
afresh where a column lands. Neither run knows the order the tools were used in, so a
channel dug after hollowing, or a blocker placed before it, came back in the wrong order.
A plate that was checked for trapped resin and sent to a printer could not be opened again
and sliced into the same file.

## Decision

A `.encrust` file holds the plate as it stands, and opening one decides nothing again.
The format is version 3, and a file of any other version is refused whole — there is no
compatibility to keep.

- Each object carries two meshes: `models/<n>/source.mesh`, the model as it was imported
  and repaired, and `models/<n>/shell.mesh`, the shell hollowing built, written only when
  there is one. The source stays so that the wall can be changed after opening.
- `built` on an object records what the run measured — the faces bounding the cavity, the
  resin it holds, the lattice it came out on, whether that lattice was coarsened, and the
  scale it was measured under — beside the wall that was asked for.
- Every support tree is in the file, the ones the automatic run grew as well as the ones a
  hand froze, in the model's own space.
- What is a cheap pure function of all that is worked out on opening and not stored: the
  bounding hierarchy, the meshes of the columns, the bodies the holes cut, the patches.
  `core_engine::project::{hollow_of, supports_of}` are the two places that do it, so the
  window, the command line and the browser open a file the same way.

`Opening` loses `hollow_budget_bytes`, and `EngineError::Hollow` goes with it: opening a
project no longer runs a cavity and so cannot fail at one.

## Consequences

A plate opens as the plate that was saved, everywhere, and `encrust slice plate.encrust`
writes the file the window would have written. `open_plate` is no longer a second
statement of the window's rules, which is what ADR 0178 warned it was.

A file is larger: a hollowed model is stored twice, and a shell is usually bigger than the
model it came from. A 400k-triangle model that was about its STL's size is now two to
three times that. Reopen this if project files become unwieldy — the source mesh is the
part to drop, at the cost of the wall no longer being editable after opening.

Changing a setting after opening rebuilds from the source in the tool's own order, as it
does for a model that was never saved. The file records no order of operations, so a
project is not a procedural history and cannot be replayed step by step.

## Alternatives considered

### Keep rebuilding, but record the order of operations

A list of operations, replayed on open, would at least come back in the right order. It is
a procedural stack — a different scene engine, and every tool would have to be expressible
as a replayable step. It also still depends on the machine's memory for the lattice.

### Store only the result, without the source mesh

Smaller files, and the same opening. But the wall, the infill and the mode could never be
changed again: a model would have to be imported afresh and every support replaced.

### The option that won, and what it costs

Project files two to three times larger, and a manifest that now records measurements
rather than only intentions — `voxel_mm` and `coarsened` mean nothing to a user and exist
only so that a hole is deepened through the same wall on opening. Supports are the one
place the line is drawn by hand: the trees are stored, their meshes are not, because
meshing a tree is deterministic and takes milliseconds.

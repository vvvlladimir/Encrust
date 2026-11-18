# 0460. A chat client names what shows, picks the recipe and brings the resin, with no window

- **Status:** Proposed
- **Date:** 2026-11-18

## Context

A language model over `encrust-mcp` (ADR 0455) beats a recipe only by what it knows and the
engine does not (`ai-integration.md`): which side of a model a person looks at, what the print
is for, which resin the user has. The window has each: the Clean surface brush paints the side
meant to be seen and Auto stands it sparser (ADR 0410, 0425), Prepare for print picks a recipe
(ADR 0403), and the start page writes a test print that reads a resin's exposure (ADR 0232).
The command line had none of the three, so a user with no window open could not finish a print
of a resin they had never set up: the server told them to create it in the window. The test
prints are built in code inside `encrust-app`; `core-engine` stood supports with no painted
side, which only the window's own path through `core-supports` took.

## Decision

- **What shows:** `SupportPlan::shown_at`, strokes of a point and a radius in plate mm as the
  model prints, paints the faces `Region::brush` reaches on the surface supports stand under;
  `core-supports` takes them as `Shown`, as the window's brush gives them. The command line
  says it as `--shown-at X,Y,Z[:R]` (4 mm), the server as `set_supports`' `shown_at_mm`.
- **Which recipe:** `list_recipes` answers `recipes list` with each recipe's TOML from
  `recipes show`; `use_recipe` keeps one as the job's `--recipe` in place of `--prepare`.
- **Which resin:** `import_resin` runs `profiles import` on a file that printed well, another
  program's export (ADR 0293, 0295) or a resin TOML. The test prints move into a crate of their
  own, **`core-samples`** on `core-geometry` alone, which the window and the command line both
  take; `encrust test-print NAME` writes one as an STL, `write_test_print` from the server.

## Consequences

A user with a file that printed well, or a resin another slicer exported, finishes a print with
no window; one with only the label's numbers still sets the resin up in the window, the test
print then telling whether the numbers hold. The painted side reaches the supports, not the
orientation search, which takes places kept clear (ADR 0458) where the pose is still open.
`core-samples` is one crate more to build, each test print in one place for both front ends.
Reopen when a resin can be set from a label's numbers alone, or when the search takes painted
points too.

## Alternatives considered

### The test prints in `core-geometry`

No new crate — but the geometry catalogue would carry the project's mark and its test plaques,
content rather than primitives.

### The painted side as the orientation's places kept clear

One mechanism instead of two — but a place kept clear is a point in the pose a client saw,
checked on every trial pose, and a painted side is faces under the pose chosen; folding one
into the other loses either the search's ranking or the brush's sizing.

### The option that won, and what it costs

Two ways to say "keep supports off this" remain — kept clear for the search, painted for the
supports — and a model must use both for a face to be spared in the pose and in the tips.

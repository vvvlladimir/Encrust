# 0354. Repair is a tool of the rail, first of Modify

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

Repair was a button: in a broken model's row and in the stage notice, running one fixed
recipe on one model. The core and the command line have since gained methods (ADR 0351),
wrapping (0352), lengths to work to and a count of crossing faces (0353); the window could
reach none of them, which breaks the parity rule (architecture rule 9) from the other side.
Repair also comes before every other change to a model: a cavity, a hole or a support built
on a broken mesh is thrown away when the mesh is repaired (`SceneObject::reshape`).

## Decision

- **`Tool::Repair` stands first in the rail's Modify group**, before Hollow, keyed 3; the
  tools after it move one digit up, Check to 9.
- Its card is heading, sections, foot (`docs/design/ui-standards.md`): **Found** sums what
  import found in the targets — holes, open edges, faces without area, faces drawn twice,
  shells — with the crossing faces counted on the section's magnifier, off the drawing
  thread, and forgotten once the mesh changes. Holes are loops of open edges, so one row
  marks both and the open edges are a count beside the shells. **Method** picks one of the six
  `core_volume::RepairMethod`s, **Settings** the seam tolerance, smallest speck and wrap
  lattice the method uses; a method that measures nothing has no Settings. The foot is
  Repair over every target, or the run's progress and Cancel.
- The method and lengths are `ToolSettings::repair`, so they are remembered and undone; a
  project does not store them, as with Relief and Combine. `RepairMethod`, `MendMethod` and
  their settings derive serde for it, as `Boolean` does. The row's and the notice's
  Repair button keep running the plain repair, to the lengths the tool is set to.
- The rail's Repair carries a dot in `warn` while a model on the plate is broken, beside
  Drain's and Check's (ADR 0218).
- A press on a finding marks it on the targets while the tool is open and brings the
  camera to it, one place a press (ADR 0355). Where they lie is `core_geometry::faults` and
  `self_intersections`, worked out off the drawing thread and forgotten with the mesh;
  nothing of it is saved.
- Faces and shells edited by hand are ADR 0356's.

## Consequences

- Every repair the command line runs is one press in the window, with the same lengths.
- A mark is a picture of what the command line already counts, so it has no flag.
- An open edge is drawn as a line `theme::MARK_LINE_W` wide, so a rim stays visible on a
  large model zoomed out.
- The digit of every tool after Measure changes; a user used to 3 for Hollow presses 4.
- The rail is nine names wide: the Russian names had to fit the widest column, so the tool
  is "Ремонт" there. A tenth tool would take the last digit, 0.
- Reopen if the dot on Repair turns out to be noise: a plate of models kept broken on
  purpose keeps it lit.

## Alternatives considered

### Keep Repair a button and put the methods in a menu beside it

No room for the findings or the lengths, and a menu of six methods with no way to say what
each does. The plan's card, its findings first, is what makes choosing a method possible.

### Put Repair in the Place group, beside Select and Measure

Place is picking and reading the plate; Repair changes the mesh, as every Modify tool does.

### The option that won, and what it costs

Renumbering the digit keys of six tools, and a ninth name on a rail already squeezed in
narrow windows.

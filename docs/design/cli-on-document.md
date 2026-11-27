# The command line on the document

The command line prepares models the way the window does: staged into one `Document`, each
shaping flag folded to an `Edit`, every edit carried out by `core_document::apply` (ADR 0458,
continuing ADR 0456). The window is the reference (ADR 0457); this page maps each
flag to its edit.

## The run

1. **Stage.** Each model is loaded, welded and checked (`pipeline::import` without
   its turn, relief or centring), then made an object by `Document::of_models`, ids from 1 in
   the order given, as the window gives them. A project is `Document::from_project`; a plate
   file's entries are models whose `position` folds to a `Move`.
2. **Fold.** The flags become `Vec<Edit>` per object in the window's order: repair and mends;
   placement (scale, orient, rotate, center, lift); relief; modelling (combine, label, cut,
   split, part-off); hollow; drains, channels, plugs; supports; arrange last, over the plate.
   `--edits FILE` appends its list after them.
3. **Apply.** Each edit through `apply` with the run's `Cancel` (Ctrl-C). An error names the
   flag it came from, as `--edits` names its index.
4. **Slice.** `into_project`, then `project::stage` and `slice_staged` as a project input is
   sliced; `--project-out` writes the project first. A run with no shaping flag folds no
   edit, so its bytes equal a plain slice's.

A recipe (`--recipe`, `--prepare`) is the same run with the fold replaced by the recipe's
steps: see [The recipe executor](#the-recipe-executor).

## Every flag

What the front end works out, from the machine it knows, goes into an edit as a final number
(ADR 0457). A steer changes how the run goes, not the plate.

| Flag | Becomes |
|---|---|
| `--weld-tolerance`, `--no-validate` | front end: the import, before the object exists |
| `--repair`, `--seam-tolerance`, `--smallest-shell` | `Repair { method, seam_tolerance_mm, smallest_shell_mm2 }` |
| `--flip-faces`, `--drop-faces`, `--drop-shells`, `--bridge` | one `Mend` |
| `--relief-image` and its flags, `--precision` | `Relief` |
| `--relief` with the file's own texture | front end: pressed at import (a `.encrust` keeps no texture) |
| `--label`, `--label-at`, `--label-height`, `--label-depth`, `--label-sunk`, `--label-follow`, `--label-font` | `Label` (`raised = !sunk`) |
| `--combine` | `Combine` over every object, `toward` unset |
| `--cut`, `--keep`, `--joint` | `Cut { lay_flat: true }`; the halves then need `--arrange` |
| `--split`, `--split-smallest` | `Split` |
| `--part-off` | `PartOff` |
| `--scale` | `Scale` |
| `--rotate` | `Rotate` |
| `--center` | front end: `Move` by the plate's middle less the object's box middle |
| `--lift` | front end: `Move` standing the object's box that high; `Orient { lift_mm }` |
| `--orient`, `--orient-goal`, `--max-tilt`, `--clean-surface` | `Orient`, `printer` and `resin` the run's catalogue ids or none, the usable area |
| `--orient-pick N` | `Orient { pick: N - 1 }` (counted from 1, decided) |
| `--orient-choices` | front end: `estimate`'s listing of the search, no edit |
| `--orient-keep-clear`, `--orient-keep-clear-turn` | `Orient { keep_clear_mm, keep_clear_turn_deg }` |
| `--hollow`, `--mode`, `--open-floor`, `--infill`, `--cell`, `--density`, `--precision` | `Hollow { settings }` |
| `--keep-solid` and its flags | `Hollow { keep_solid }`, fitted in `core-document` |
| `--drain-at X,Y,Z`, `--drain`, `--drain-depth`, `--drain-taper`, `--drain-sides` | `Drain { at_mm, size, kind: AtDepth }`, points in plate mm where the object stands |
| `--walled` | `Drain { kind: Walled }`, folded before the `Hollow` so the cavity keeps a wall round it |
| `--drain-at auto`, `--no-vent` | `Drain { auto: true, vent }`, no lift planned first |
| `--channel`, `--channel-mm` | `Channel` |
| `--drain-plugs`, `--plug-gap` | `Plugs { gap_mm: plug_gap - drift, plate_mm, low_mm }` |
| `--drain-support-at` | `Drain { kind: Support, at_mm }` into the raft |
| `--supports NAME`, `--support-profile FILE` | `Supports { profile }`; a file or user-folder profile is written as a plate group first |
| `--support-cavity`, `--support-at`, `--support-only-at`, `--shown-at`, `--clean-surface` | `Supports { cavity, at_mm, only_at, shown_at, clean_surface }` |
| `--merge-supports` | `MergeSupports { object, near_mm }` |
| `--arrange` | `Arrange` over the usable area from the printer's inset, the window's spacing |
| `--printer`, `--resin`, `--profile`, `--material` | front end: the machine, and the numbers above |
| `--layer-height`, `--exposure-at`, `--adaptive` and its kin, raster flags, `--ctb-version` | front end: the slice |
| `--remove-islands`, `--hanging-passes`, `--check-drainage` | front end: the slice and its report |
| `--prepare`, `--recipe` | the recipe executor |
| `--strict`, `--project-out`, `--edits`, `--plate`, `-o` | steer |

## Where the window's way sets the behaviour

- Holes stand on the model where it stands and supports grow under what stands; no lift is
  planned from the profile first.
- `Orient` turns the shell, cuts and vents as they stand, its trial on the printer named or
  the standard one; `--orient-pick` counts from 1 and the fold subtracts 1.
- `Arrange` runs last over the usable area, footprints with their supports; a model that does
  not fit is an info naming it, not an error.
- Drain and channel points are plate mm where the object stands when the edit runs.
- A plug's gap is `gap - drift`, the plug beside its hole's object.
- `Hollow` replaces the cavity as the window does; flags override a recipe step's keys.

## The recipe executor

`recipe::lay` keeps its loop: `decide` per model per step, `Run`, `Skip`, `Stop`, `Pause`,
the report unchanged. A plate step (`repair`, `orient`, `hollow`, `drain`,
`supports`, `arrange`) is `core_recipe::edits_of` over the step's settings merged with the
flags (`folded`), then `apply` on the run's `Document`; `Piece` keeps only what the report
and the metrics read, measured off the object after the edit. `check`, `save`, `gate` and
`pause` stay steps of the executor: Check's retries and its fixes (cups, levers, islands) run
on the document's objects, a save slices the document as it stands. `--recipe-out` is
`folded`.

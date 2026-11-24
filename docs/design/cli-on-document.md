# The command line on the document

The command line prepares models the way the window does: staged into one `Document`, each
shaping flag folded to an `Edit`, every edit carried out by `core_document::apply` (ADR 0472,
continuing ADR 0466's phase 5). The window is the reference (ADR 0471); this page says what
the command line does to reach it, and the cards that got it there, all done.

## The run

1. **Stage.** Each model is loaded, welded and checked as today (`pipeline::import` without
   its turn, relief or centring), then made an object by `Document::of_models`, ids from 1 in
   the order given, as the window gives them. A project is `Document::from_project`; a plate
   file's entries are models whose `position` folds to a `Move`.
2. **Fold.** The flags become `Vec<Edit>` per object in the window's order: repair and mends;
   placement (scale, orient, rotate, center, lift); relief; modelling (combine, label, cut,
   split, part-off); hollow; drains, channels, plugs; supports; arrange last, over the plate.
   `--edits FILE` appends its list after them.
3. **Apply.** Each edit through `apply` with the run's `Cancel` (Ctrl-C). An error names the
   flag it came from, as `--edits` names its index today.
4. **Slice.** `into_project`, then `project::stage` and `slice_staged` as a project input is
   sliced today; `--project-out` writes the project first. A run with no shaping flag folds no
   edit, so its bytes equal today's.

A recipe (`--recipe`, `--prepare`) is the same run with the fold replaced by the recipe's
steps: see [The recipe executor](#the-recipe-executor).

## Every flag

What the front end works out, from the machine it knows, goes into an edit as a final number
(ADR 0471). A steer changes how the run goes, not the plate.

| Flag | Becomes |
|---|---|
| `--weld-tolerance`, `--no-validate` | front end: the import, before the object exists |
| `--repair`, `--seam-tolerance`, `--smallest-shell` | `Repair { method, seam_tolerance_mm, smallest_shell_mm2 }` |
| `--flip-faces`, `--drop-faces`, `--drop-shells`, `--bridge` | one `Mend` |
| `--relief-image` and its flags, `--precision` | `Relief` |
| `--relief` with the file's own texture | front end: pressed at import, as now (a `.encrust` keeps no texture) |
| `--label`, `--label-at`, `--label-height`, `--label-depth`, `--label-sunk`, `--label-follow`, `--label-font` | `Label` (`raised = !sunk`) |
| `--combine` | `Combine` over every object, `toward` unset |
| `--cut`, `--keep`, `--joint` | `Cut { lay_flat: true }`; the halves then need `--arrange` as now |
| `--split`, `--split-smallest` | `Split` |
| `--part-off` | `PartOff` |
| `--scale` | `Scale` |
| `--rotate` | `Rotate` |
| `--center` | front end: `Move` by the plate's middle less the object's box middle |
| `--lift` | front end: `Move` standing the object's box that high; `Orient { lift_mm }` |
| `--orient`, `--orient-goal`, `--max-tilt`, `--clean-surface` | `Orient`, `printer` and `resin` the run's catalogue ids or none (P8), the usable area (B2) |
| `--orient-pick N` | `Orient { pick: N - 1 }` (counted from 1, decided) |
| `--orient-choices` | front end: `estimate`'s listing of the search, no edit |
| `--orient-keep-clear`, `--orient-keep-clear-turn` | `Orient { keep_clear_mm, keep_clear_turn_deg }` |
| `--hollow`, `--mode`, `--open-floor`, `--infill`, `--cell`, `--density`, `--precision` | `Hollow { settings }` |
| `--keep-solid` and its flags | `Hollow { keep_solid }`, fitted in `core-document` |
| `--drain-at X,Y,Z`, `--drain`, `--drain-depth`, `--drain-taper`, `--drain-sides` | `Drain { at_mm, size, kind: AtDepth }`, points in plate mm where the object stands |
| `--walled` | `Drain { kind: Walled }`, folded before the `Hollow` so the cavity keeps a wall round it |
| `--drain-at auto`, `--no-vent` | `Drain { auto: true, vent }`, no lift planned first |
| `--channel`, `--channel-mm` | `Channel` |
| `--drain-plugs`, `--plug-gap` | `Plugs { gap_mm: plug_gap - drift, plate_mm, low_mm }` (B3, P6) |
| `--drain-support-at` | `Drain { kind: Support, at_mm }` into the raft |
| `--supports NAME`, `--support-profile FILE` | `Supports { profile }`; a file or user-folder profile is written as a plate group first (P3) |
| `--support-cavity`, `--support-at`, `--support-only-at`, `--shown-at`, `--clean-surface` | `Supports { cavity, at_mm, only_at, shown_at, clean_surface }` |
| `--merge-supports` | `MergeSupports { object, near_mm }` |
| `--arrange` | `Arrange` over the usable area from the printer's inset, the spacing as now |
| `--printer`, `--resin`, `--profile`, `--material` | front end: the machine, and the numbers above |
| `--layer-height`, `--exposure-at`, `--adaptive` and its kin, raster flags, `--ctb-version` | front end: the slice |
| `--remove-islands`, `--hanging-passes`, `--check-drainage` | front end: the slice and its report |
| `--prepare`, `--recipe` | the recipe executor |
| `--strict`, `--project-out`, `--edits`, `--plate`, `-o` | steer |

## Where the command line differs today, and the window's way

| Item | Today | From now on |
|---|---|---|
| 2 | supports and holes placed for a lift worked out from the profile first (`stage/ground.rs`) | no planned lift: holes on the model where it stands, supports grow under what stands; `check_lift` goes |
| 8 | orient on the bare model | `Orient` on shell, cuts and vents as they stand (the edit's own); the trial on the printer named or the standard one (decided) |
| 10 | `--orient-pick` from 1 | kept from 1; the fold subtracts 1 (decided) |
| 11–13 | arrange before supports, the whole plate, a no-fit an error | `Arrange` last, footprints with their supports, the usable area; a no-fit is an info naming what was left out |
| 16–17 | drain and channel points in the imported model's frame | plate mm where the object stands when the edit runs |
| 19 | plug gap typed raw, plugs in a row | `gap - drift`, plugs at P6's spot |
| 22 | a hollow step reshells the shell | `Hollow` replaces the cavity as the window does; flags override a recipe step's keys, gate and pause as now |

## The recipe executor

`recipe::lay` keeps its loop: `decide` per model per step, `Run`, `Skip`, `Stop`, `Pause`
as now, the report unchanged. A plate step (`repair`, `orient`, `hollow`, `drain`,
`supports`, `arrange`) is `core_recipe::edits_of` over the step's settings merged with the
flags (`folded`), then `apply` on the run's `Document`; `Piece` keeps only what the report
and the metrics read, measured off the object after the edit. `check`, `save`, `gate` and
`pause` stay steps of the executor: Check's retries and its fixes (cups, levers, islands) run
on the document's objects, a save slices the document as it stands. `--recipe-out` is
`folded` as now.

## Deleted and kept

Deleted: `stage.rs`'s `put_down`, `put_down_every`, `Laid`, `imported`'s turn; `pipeline.rs`'s
`hollow_and_cut` and `drill_auto`; `pipeline/place.rs`'s orient search; `hollowing.rs` and
`hollowing/` but the flag structs; `supports.rs` but `SupportReport`; `stage/ground.rs`;
`stage/room.rs`'s stood-at, a plate file's position now a `Move` (its spread stays for a
recipe's slide-clear); `modelling.rs`'s calls (the flags stay);
`recipe/{steps,plate,piece,drain}.rs` where `apply` does the step; `plate_edits::stage_models`'s
refusals.

Kept: load, weld, check and its report; slicing, writing, `estimate`'s measure, the batch,
`render`, the JSON report (each `Part` filled from the object and the edit's `Report`).

## Cards

Each card is a commit, at most ~10 files, a closed-form test and one cross-check: the flags
and the same edits through `--edits` give equal `.encrust` files (`--project-out`, compared
by manifest and mesh bytes).

| Card | What | Test |
|---|---|---|
| R6d1 | stage unshaped models into a `Document`, their models from `core_engine::plate_models` | a 10 mm cube: its layers equal its `--project-out` project's; a plain run's PNG stack and JSON unchanged |
| R6d2 | modelling, mend and repair flags fold to edits; `Repair`'s two limits | `--cut z:5` on a 10 mm cube: two pieces of 500 ± 0.01 mm³; flags = `--edits` |
| R6d3 | placement flags (`--scale`, `--rotate`, `--center`, `--orient*`); `Orient`'s keep-clear | a 10 × 10 × 30 cuboid `--orient --orient-goal shortest-print`: height 10 ± 1e-3 |
| R6d4 | hollow with keep-solid, drains (raft holes too), channels, plugs | a 20 mm cube `--hollow 2 --drain-at 10,10,0`: one `AtDepth` hole at the floor ± 1e-3; plugs `3 - 2 × (gap - drift)` across |
| R6d5 | supports flags; a file profile as a plate group | `--supports` on a cube lifted 5 mm: contacts equal `--edits`' |
| R6d5b | `MergeSupports` and `--merge-supports` | two contacts 2 mm apart merged: one tree, its foot under their middle ± 1e-3 |
| R6d6 | `--arrange` last over the usable area | two 10 mm cubes on an inset plate: every footprint inside ± 1e-3 |
| R6d7 | the recipe executor's plate steps through `edits_of` + `apply` | `--prepare` on a 20 mm cube: the same edits in its report as `edits_of` gives |
| R6d8 | delete the old pipeline; `cli.md`, `features.md`, `architecture.md` | the suite green, `xtask hygiene --bless` |

## Decided (2026-11-21)

Every flag with no edit yet gets one; none is refused or kept beside the document.

1. `Repair` gains `seam_tolerance_mm` and `smallest_shell_mm2`, the window's where unset (R6d2).
2. `Orient` gains `keep_clear_mm` points and the turn they are judged at, which MCP's `Rotate`
   handover gives way to (R6d3).
3. `Hollow` gains the keep-solid runs, fitted to the model in `core-document` by the code moved
   down from `encrust-cli/src/keep_solid.rs` (R6d4).
4. `Drain { kind: Support }` drills the raft at plate points (R6d4).
5. A `MergeSupports { object, near_mm }` edit, its own variant as the window's Merge into one is
   its own action (R6d5b).

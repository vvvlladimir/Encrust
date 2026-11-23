# Plan: one plate document, one `apply`, the window owns it when open

Goal and reasons: ADR 0466 (written in M0). This file is the work list: one card = one
implementer run + one reviewer run + one commit. Cards run **one after another** (one cargo at
a time; a worktree each would rebuild the whole target).

## Who does what

| Step | Who | Why |
|---|---|---|
| M0, M1 | orchestrator (Opus), by hand | the decisions and the worked example every card copies |
| C-cards | `implementer` (Sonnet) | same shape repeated, all context in the card |
| each card | `reviewer` (Opus, medium) | reads the diff only, ≤ 25 lines out |
| milestone end | `reviewer` on the whole range | catches what single cards cannot |

Loop per card: implementer → reviewer → (FIX: back to the **same** implementer via
SendMessage, once) → PASS: commit. A second FIX or an ESCALATE comes to the orchestrator.

## Card format

```
### C<n>. <action>
Read:  files the implementer needs (≤ 8)
Touch: files it may change; anything else is a stop
Move:  old fn/path → new path
Edit:  variants wired
Done when: tests named, with their closed forms; unshown.rs lines removed
Not now: what the next card does
```

## M0 — foundations (orchestrator)

1. ADR 0466 draft, `docs/design/document.md`, `docs/architecture.md` edges. Reviewed by the
   user before code.
2. `core-edit`: `Edit` with `Cut` alone, serde + round-trip proptest. Each card adds its own
   variants (no variant before its `apply`). `Slicing`/`Machine` stay out of `Edit`.
3. `core-document`: `Document`, `from_project`/`into_project`, `ObjectId` in core-engine's
   manifest (`#[serde(default)]`, no version bump, ids given on read); the window's
   `ObjectId` is that one, its ids start at 1. `apply` arrives with `Cut` in M1, taking
   cancel and progress because window jobs need both.
4. Manifests, `cargo hakari generate && cargo hakari manage-deps`, full check once,
   `cargo xtask arch`, `cargo sweep --time 7`.

## M1 — the vertical slice on `Cut` (orchestrator)

The worked example. After it, the user's case works for a cut.

1. `apply` for `Cut`: moved from `encrust-cli/src/modelling.rs::cut`/`stood` and
   `encrust-app/src/job/cut.rs`; closed-form test: a 10 mm cube cut at its middle → two
   halves of 500 ± 0.01 mm³ (f32 over the cut faces). Front ends call `cut` + `pieces` until
   the Scene is a Document (phase 6); `Run` joins `apply` with Hollow (C9b).
2. CLI: `--cut` folds to `Edit::Cut`; `--edits FILE`, `--project-out FILE`, `.encrust` input
   with edits allowed (`project.rs:200`).
3. Window: the cut tool builds `Edit::Cut`, runs `apply` on its worker; one undo entry.
4. Link schema 3: `version`, `want: plate`, `apply` + `base_version`, `stale`, one retry.
5. MCP: depends on `core-edit`; a revision = base `.encrust` + `N.edits.json` for `Cut`;
   other actions still go the old way.
6. Tests from the plan's Verification for cut + undo; manual run with `Gta2.3mf`.

## C-cards (implementer), in this order

Each card wires one group through all four places — core, CLI, window, MCP — and deletes its
lines from `crates/encrust-mcp/src/recipe/unshown.rs`. A file over the limit is split first,
in its own card.

| Card | Edits | Move from (CLI / window) |
|---|---|---|
| C1 | Move, Rotate, Scale | `pipeline/place.rs` / `job/place.rs` |
| C2 | Delete, Duplicate, Arrange | `modelling.rs`, `stage/room.rs` / `job/plate.rs`, `job/edit.rs` |
| C3 | Split, PartOff | `modelling.rs::parted` / `job/split.rs` |
| C4 | Combine | `modelling.rs::combined` / `job/boolean.rs` |
| C5 | Label | `modelling.rs::labelled` / (window label tool) |
| C6 | Repair, Mend | `pipeline/checked.rs` / `job/repair.rs` |
| C7 | Relief | `pipeline/relief.rs` / `job/relief.rs` |
| C8 | Orient | `pipeline.rs` orient part / `job/orient.rs` (+`seen.rs`) |
| C9a | split `hollowing.rs` (627 lines) first | — |
| C9b | Hollow | `hollowing.rs`, `keep_solid.rs` / `job/hollow.rs`, `hollow/fields.rs` |
| C10 | Drain | `recipe/drain.rs`, `hollowing` drains / `job/drains.rs`, `drill.rs`, `plugs.rs` |
| C11 | Supports, ShownAt | `supports.rs` / (support jobs) |
| C12 | core-recipe steps yield `Edit`s; delete `encrust-cli/src/recipe/{steps,plate}.rs`, `encrust-app/src/prepare/{work,step}.rs` |
| C13 | MCP: `set_*` append edits, `revert` truncates; delete `unshown.rs`, `flags()` path, `N.recipe.toml` |
| C14 | snapshots retaken (ADR 0215), `docs/cli.md`, `features.md`, `window-link.md`, `ai-integration.md`, 0459 status |

The orchestrator writes each card in full (Read/Touch/Move/Done when) just before running it,
from the code as it is then — not all now, since C1 changes what C2 reads.

## Token rules

- The card holds the context; the implementer reads only its "Read" list and the M1 example.
- Reports are capped (12 lines implementer, 25 reviewer); the orchestrator does not read diffs.
- Cargo output through `tail -n 30`; per-crate checks only.
- Two failed tries → stop and report, never a third approach.
- Milestone review (after C5, C11, C14): one reviewer run over the range, high effort.

## Cards in full

### C1. Move, Rotate, Scale
Read: `crates/core-edit/src/edit.rs` (+`edit/tests.rs`), `crates/core-document/src/{apply.rs,
  apply/tests.rs,cut.rs,piece.rs,document.rs,fixtures.rs}`, `crates/encrust-cli/src/pipeline/place.rs`
  (`euler_degrees` only), `crates/encrust-app/src/link/plate.rs`, `crates/encrust-app/src/link/tests/plate.rs`
Touch: the files above, new `crates/core-document/src/placement.rs` (+ `placement/tests.rs` if
  past 150 test lines), `crates/core-document/src/lib.rs`, `crates/core-document/src/error.rs`,
  `docs/design/document.md`, `docs/cli.md` (one sentence in the `--edits` paragraph)
Move: `euler_degrees` from `encrust-cli/src/pipeline/place.rs` → `core-document/src/placement.rs`
  (pub, re-exported); the CLI imports it from `core_document`.
Edit: three variants in `core-edit`, fields serde as the cut's (`#[serde(tag = "edit")]`, kebab):
- `Move { object, by_mm: Vec3 }` — translation += by_mm.
- `Rotate { object, degrees: Vec3 }` — the object's rotation SET to `euler_degrees(degrees)`
  (X, then Y, then Z about the plate's axes, as `--rotate`), turned about its centre of mass on
  the plate, then stood on the plate (lowest point at z 0).
- `Scale { object, factors: Vec3 }` — the object's scale SET to `factors`, about its centre of
  mass, then stood on the plate. A factor not finite or ≤ 0 is `ApplyError::BadScale(ObjectId)`.
In `placement.rs`: `pub fn moved(Transform, Vec3) -> Transform`, `pub fn turned_to(Transform,
  &Mesh, Vec3) -> Transform`, `pub fn sized_to(Transform, &Mesh, Vec3) -> Result<Transform, ()>`
  (the mesh is the object's own; use `core_geometry::{center_of_mass, transform_mesh,
  drop_to_plate}`). `apply` runs them on `ObjectState.transform`; `Report::Placed` (no fields);
  `Applied { removed: [], added: [] }`.
Window: in `link/plate.rs`, `start_edit` returns `Result<Option<ObjectId>, String>`: the cut
  `Some(object)` as now; the three new edits set `SceneObject::transform` at once through the
  same `placement` fns on `object.mesh`, and answer `None` (nothing under way).
Done when:
- core-edit: the round-trip proptest covers the new variants; a hand-written `{"edit":"rotate",
  "object":1,"degrees":[90,0,0]}` reads.
- core-document, on the 10 mm cube fixture: `Move` by (5,0,0) → bounds x 5..15 ± 1e-4;
  `Rotate` (90,0,0) → bounds still 10 × 10 × 10 ± 1e-4 with min z 0 ± 1e-4; `Scale` (2,1,1) →
  volume 2000 ± 0.01 mm³ and min z 0; `Scale` (0,1,1) → `BadScale`, transform unchanged.
- window link test: `apply` of `Rotate` (0,0,90) on the bench cube answers `applied: true`
  and the object's rotation is 90° about Z (angle ± 1e-4).
- `cargo test -p core-edit -p core-document -p encrust-cli -p encrust-app` (one at a time),
  clippy per crate, `cargo fmt --all`.
Not now: MCP wiring (C1b, orchestrator), Orient (C8), the window's own position panel.

### C2. Delete, Duplicate, Mirror (and `plug_of` kept right)
Read: `crates/core-document/src/{document.rs,apply.rs,apply/tests.rs,fixtures.rs}`,
  `crates/core-engine/src/project.rs` (`ObjectState::plug_of`, an index into `objects`),
  `crates/encrust-app/src/scene.rs` (`duplicate`, `mirror`, `remove`), `crates/encrust-app/src/link/plate.rs`
Touch: those, `crates/core-edit/src/edit.rs` (+tests), new `crates/core-document/src/plate_contents.rs`,
  `crates/core-document/src/lib.rs`, `crates/encrust-app/src/link/tests/plate.rs`, `docs/design/document.md`
Fix first (its own commit, test failing before): `Document::replace` removes and inserts objects,
  but `plug_of` is an index, so a plug of a later object then names the wrong one. Every remove
  or insert in `Document` goes through two private helpers that renumber `plug_of`: an index
  past the change shifts, one naming the removed object becomes `None`. Test: a plate of two
  cubes and a plug of the second (`plug_of: Some(1)`); cutting the first leaves the plug naming
  the second cube at its new index; cutting the second leaves the plug's `plug_of` `None`.
Edit, in `core-edit`:
- `Delete { object }` — removed; its plugs (objects whose `plug_of` names it) go with it.
- `Duplicate { object, offset_mm: Vec3 }` — a copy with all its state (supports, cavity, own
  settings) and meshes (`Arc` clones), a new id, `plug_of: None`, translation += offset,
  inserted right after the original. `Applied.added = [(id, Role::Copy)]` (new `Role` variant).
- `Mirror { object, axis: Axis }` — scale on that axis of the object's own frame negated,
  then moved back so its bounds' centre is where it was (as `Scene::mirror`).
Window: `start_edit` maps them to `Scene::remove` (and its plugs), `Scene::duplicate`,
  `Scene::mirror`, answering `None` (nothing under way).
Done when: core tests on the 10 mm cube fixture — `Delete` leaves no object and `NoObject`
  for a missing id; `Duplicate` by (20,0,0) leaves two objects, the copy's bounds x 20..30 ±
  1e-4, its id above every other; `Mirror` X keeps bounds 0..10 on every axis ± 1e-4 and the
  volume 1000 ± 0.01 (winding fixed by `transform_mesh`); the `plug_of` test above; one window
  link test applying `Duplicate`. Per-crate test and clippy, `cargo fmt --all`.
Not now: Arrange (C2b: needs the plate's area and supports' footprints), MCP wiring.

### C3. Split, PartOff (and `apply` takes a `Cancel`)
Read: `crates/encrust-app/src/job/split.rs`, `crates/encrust-app/src/cut.rs` (`start_split`,
  `land_parts`, `land_parted_off`), `crates/encrust-cli/src/modelling.rs` (`parted`),
  `crates/core-document/src/{apply.rs,cut.rs,piece.rs,error.rs}`, `crates/core-volume/src/cancel.rs`
Touch: those, `crates/core-edit/src/edit.rs` (+tests), new `crates/core-document/src/split.rs`,
  `crates/core-document/src/lib.rs`, `crates/core-document/src/apply/tests.rs`,
  `crates/encrust-cli/src/plate_edits.rs`, `crates/encrust-app/src/job/split.rs`,
  `crates/encrust-app/src/link/plate.rs` (+ its test), `docs/design/document.md`
Signature: `apply(&mut Document, &Edit, Cancel<'_>)` with `core_volume::Cancel` (it carries the
  progress too): this is ADR 0466's `Run`. Callers that cannot cancel pass `Cancel::never()`.
Edit, in `core-edit`:
- `Split { object, smallest_mm3: Option<Scalar> }` — `core_volume::parts` with
  `PartSettings { smallest_mm3 }` (default when `None`); one part and no scrap joined or dropped
  leaves the object as it was (`Report::Split { parts, joined, dropped }`).
- `PartOff { object, at_mm: Vec3 }` — `core_volume::part_off` at that plate point.
Both run on the object's printed mesh placed on the plate (as `SplitRequest::placed`), and
  replace it with `Piece::sound(Role::Part(n), mesh)` (new `Role::Part(usize)`, from one) through
  `Document::replace`. Errors: `NoNeck | NotOnModel` → `ApplyError::NoNeck(ObjectId)`;
  `Cancelled` → `ApplyError::Cancelled`; any other → `ApplyError::Failed { object, reason }`.
Move: `job/split.rs`'s `run` becomes a call of core-document's split and part-off functions
  (the window keeps its worker, its `tr!` names and its landing); `modelling::parted` calls the
  same functions. Nothing of the two computations is left in the front ends.
Done when: core tests — two 10 mm cubes 5 mm apart in one mesh (`core_geometry::cuboid` twice,
  appended) split into 2 parts of 1000 ± 0.01 mm³ each; one cube splits into nothing new (object
  unchanged, same id); `PartOff` at a point off the model → `NoNeck`; a `Cancel` already
  stopped → `Cancelled` and the document unchanged. Window link test applying `Split` on two
  cubes. All existing cut, split and CLI `--split`/`--part-off` tests still pass.
Not now: Combine (C4), MCP wiring.

### C4. Combine
Read: `crates/encrust-app/src/job/boolean.rs`, `crates/encrust-app/src/boolean.rs` (its landing),
  `crates/encrust-cli/src/modelling.rs` (`combined`), `crates/core-document/src/{apply.rs,document.rs,split.rs}`
Touch: those, `crates/core-edit/src/edit.rs` (+tests), new `crates/core-document/src/combine.rs`,
  `crates/core-document/src/{lib.rs,error.rs,apply/tests.rs}`, `crates/encrust-app/src/link/plate.rs`
  (+ test), `docs/design/document.md`
Edit: `Combine { objects: Vec<ObjectId>, op: Op, toward: Option<Vec3> }`, `Op` = union |
  difference | intersection (kebab, in core-edit; `core_volume::Boolean` from it). The first
  object is what a difference keeps. `toward` set: `combine_exact_toward`, else `combine`
  (Method::ExactToward / Sampled of the window). Fewer than two objects or one missing:
  `ApplyError::NoObject`/new `TooFew`. `NothingLeft` → `ApplyError::NothingLeft`; `NotSolid` →
  `ApplyError::NotSolid(ObjectId)` (the first); other → `Failed`.
Move: the body of `job/boolean.rs::run` becomes core-document's `combined(placed, op, toward,
  Cancel)`; the window maps its `ApplyError` to its `tr!` strings; `modelling::combined` calls it.
Result: the objects are replaced by one `Piece::sound(Role::Combined, mesh)` at the first's
  index (`Document::replace` for the first, `Delete`'s helper for the rest).
Done when: two 10 mm cubes overlapping by 5 mm on x → union 1500 ± 0.01 mm³, difference 500 ±
  0.01, intersection 500 ± 0.01; two apart → intersection is `NothingLeft`; one object →
  `TooFew`. Window link test applying a union. Existing boolean, text and `--combine` tests pass.
Not now: Label (C5), MCP wiring.

### C5. Label
Read: `crates/encrust-cli/src/modelling.rs` (`labelled`, `BUNDLED_FONT`), `crates/encrust-app/src/text.rs`
  (`label_at`, `stamp`), `crates/core-text/src/lib.rs`, C4's `combine.rs`
Touch: those, `crates/core-edit/src/edit.rs` (+tests), new `crates/core-document/src/label.rs`,
  `crates/core-document/{Cargo.toml (+core-text), src/lib.rs, src/error.rs, src/apply/tests.rs}`,
  `docs/architecture.md` (the core-document edge to core-text), `crates/encrust-app/src/link/plate.rs` (+ test)
Edit: `Label { object, text: String, at_mm: Vec3, height_mm, depth_mm, raised: bool,
  follow_surface: bool, angle_deg, font: Option<PathBuf> }` (defaults as the CLI's flags:
  `LETTER_HEIGHT_MM`, `LABEL_DEPTH_MM`, raised, not following, 0°). `font: None` is the bundled
  IBM Plex Sans: `BUNDLED_FONT` moves from the CLI into `label.rs`.
Move: `modelling::labelled`'s outline-frame-label steps become core-document's
  `labelled(mesh, transform, &LabelAsked, cancel) -> Result<Mesh, ApplyError>`, stamped with C4's
  `combined` (`toward` = the face's normal, as the window); `modelling::labelled` calls it. The
  window's Text tool keeps its own fonts and ghost; only an edit from the link uses `labelled`.
Errors: blank text, no surface near `at_mm`, a font that does not read → new `ApplyError`
  variants, each tested.
Done when: "I" raised 1 mm on the 10 mm cube's top face adds volume (> 1000 + 1 mm³), sunk
  removes it (< 1000 − 1); each new error variant has a test; window link test stamping one.
Not now: the window's font picker over the link, MCP wiring.

### C6. Repair, Mend
Read: `crates/encrust-app/src/job/{repair.rs,edit.rs}`, `crates/encrust-app/src/scene/object.rs`
  (`reshape`), `crates/encrust-cli/src/{edits.rs,pipeline/checked.rs}` (`repaired`,
  `watched_repair`), `crates/core-document/src/{apply.rs,document.rs,error.rs}`
Touch: those, `crates/core-edit/src/edit.rs` (+tests), new `crates/core-document/src/repair.rs`,
  `crates/core-document/src/{lib.rs,apply/tests.rs}`, `crates/encrust-app/src/link/plate.rs`
  (+ test), `docs/design/document.md`, `docs/cli.md` and `docs/features.md` (name the two edits
  where `--edits` lists the others)
Edit:
- `Repair { object, method: RepairMethod }` — `core_volume::repair` with the window's default
  `RepairSettings`, on the object's source mesh; `RepairMethod` from core-volume (serde kebab;
  add the derives there if missing).
- `Mend { object, flip_faces: Vec<usize>, drop_faces: Vec<usize>, drop_shells: Vec<usize>,
  bridge: Vec<[[u32; 2]; 2]> }` — numbered as `--flip-faces` and the window's Repair list number
  them; turns and bridges first, drops last (as `edits::apply`).
Both replace the object's source mesh and drop what was built on the old surface — supports,
  cavity, painted regions, plugs of it — as `SceneObject::reshape` does: one
  `Document::reshape(id, Mesh)` holds that rule.
Move: `edits::apply`'s body → core-document `mended(&mut Mesh, &MendAsked) -> Result<Mended,
  ApplyError>` (the CLI keeps only flag→asked and its message); `watched_repair`'s call and
  `job/repair.rs::run`'s call → core-document `repaired(mesh, method, Cancel)`. Errors: a face or
  shell number past the last → `ApplyError::NoSuchFace { object, number }` / `NoSuchShell`; a
  bridge the mesh refuses → `ApplyError::Failed`; `Cancelled`.
Done when: a 10 mm cube with one face's winding turned (fixture) mended by `flip_faces [n]` is
  closed and orientable, volume 1000 ± 0.01; `Mend` with face 13 on a 12-face cube →
  `NoSuchFace`; shell 2 on one shell → `NoSuchShell`; `Repair` of a cube with a face removed
  closes it; a stopped `Cancel` → `Cancelled`. Window link test of `Mend`. Existing repair,
  `--repair`, `--flip-faces`, `--bridge` tests pass.
Not now: Relief (C7), MCP wiring.

### C7. Relief (a picture tiled; after C6, which adds `Document::reshape`)
Read: `crates/encrust-app/src/job/relief.rs` (`run`, `pressed`), `crates/encrust-cli/src/pipeline/relief.rs`
  (`press_relief`, `read_picture`), `crates/core-document/src/{apply.rs,document.rs,repair.rs}`
Touch: those, `crates/core-edit/src/edit.rs` (+tests), new `crates/core-document/src/relief.rs`
  (+ `relief/tests.rs`), `crates/core-document/{Cargo.toml if a picture reader is needed, src/lib.rs,
  src/error.rs}`, `crates/encrust-app/src/link/plate.rs` (+ test), `docs/design/document.md`,
  `docs/cli.md` and `docs/features.md` (name the edit where `--edits` lists the others)
Edit: `Relief { object, depth_mm, image: PathBuf, tile_mm, angle_deg, invert, symmetric,
  smoothing_px, hold_bottom_deg, hold_top_deg, precision }` (options default as the CLI's flags).
  Only a picture tiled over the model: a `.encrust` keeps no file texture to press.
Move: the tiled branch of `job/relief.rs::pressed` and of `press_relief` → core-document
  `relieved(mesh, &ReliefAsked, painted: Option<&Region>, Cancel) -> Result<Relieved,
  ApplyError>` over `press_tiled`, the object's `relief_painted` region limiting it as the window's
  brush does. The picture is read where the CLI reads it now (`read_picture` moves down with it;
  a new dependency needs asking first — stop and report if one is needed). Result through
  `Document::reshape`. The window's job keeps the file-texture branch; its tiled branch calls core.
Errors: a picture that does not read → `ApplyError::BadPicture { path, reason }`; `Cancelled`;
  others → `Failed`.
Done when: a 10 mm cube pressed 0.5 mm with a black-and-white checker picture (written by the
  test into a temp file) changes its volume by more than 1 mm³ and keeps its bounds within
  ± 0.6 mm of 0..10; a picture path that is not there → `BadPicture`; a stopped `Cancel` →
  `Cancelled`. Window link test of `Relief`. Existing relief and `--relief` tests pass.
Not now: the file texture's relief over the link, MCP wiring.

### C8. Orient — not an edit (decided)
`orient_for_print` is already the one call the window and `--orient` make (ADR 0290). What the
window lacks is the pose a client's check chose, and that is a `Rotate` (C1). The MCP wiring
hands the chosen pose over as `Edit::Rotate`; no `Orient` variant. `docs/design/document.md`'s
Edits table drops Orient with a line saying so.

### C9. Hollow
Read: `crates/encrust-app/src/hollow.rs` (`settings_of`, `settings_for`, the tool's start of a
  `HollowJob`), `crates/encrust-app/src/job/hollow.rs` (`HollowTask::of`, `shell_of`, `again_tasks`),
  `crates/encrust-app/src/project/state.rs` (`built_cavity`), `crates/core-engine/src/project.rs`
  (`BuiltCavity`, `Cavity`, `HollowState`) and `project/restore.rs` (`hollow_of`),
  `crates/core-document/src/{apply.rs,document.rs,split.rs}`
Touch: those, `crates/core-edit/src/edit.rs` (+tests), new `crates/core-document/src/hollow.rs`
  (+ `hollow/tests.rs`), `crates/core-document/src/{lib.rs,error.rs}`, `crates/encrust-app/src/link/plate.rs`
  (+ test), `docs/design/document.md`, `docs/cli.md` and `docs/features.md` (name the edit)
Edit: `Hollow { object, settings: HollowState }` — the object keeps `settings` as its own
  (`own_hollow`) and is hollowed by them: the window's numbers, the window's rule.
Move (one home each):
- `settings_of(&HollowState, Transform) -> HollowSettings` from encrust-app/src/hollow.rs →
  core-document/src/hollow.rs; the window's `settings_for` calls it.
- `built_cavity`'s body → core-engine `BuiltCavity::of(&Shell)` (a pure conversion beside the
  type, as architecture rule 4 allows); the window's `built_cavity` calls it with its shell.
In core-document: `shelled(mesh, bvh, &HollowSettings, shape: Mat3, Cancel) -> Result<Shell,
  ApplyError>` = `cavity_field` then `carve` (the window's `shell_of` without its field cache,
  which stays in the window). `apply`: `hollow_of` the object's state, `asking(settings_of(..))`,
  `shelled` on its source mesh with `transform.unturned()`, then the shell into
  `ModelMeshes::shell` and `BuiltCavity::of` into `ObjectHollowState::built`; supports stay.
Window: the link's Hollow arm (a helper, `start_edit` under 80 lines) sets the object's own
  hollow numbers and starts the tool's `HollowJob` for that object alone (as `again_tasks`
  does), under way until it lands, as the cut.
Errors: `Cancelled`; a cavity with no room (`VolumeError` the field gives for a wall thicker
  than the model) → `ApplyError::NoRoom(ObjectId)`; others → `Failed`.
Done when: a 20 mm cube hollowed with a 2 mm wall keeps 20³ − 16³ = 3904 ± 40 mm³ of material
  (the lattice's error, stated in the message) and its `built` carries the wall; a 2 mm cube
  with a 2 mm wall → `NoRoom` (or, if the field gives a solid instead, the test says what it
  gives and the variant is not added); a stopped `Cancel` → `Cancelled`; the plate written and
  read back keeps the shell. Window link test of `Hollow`. Existing hollow tests pass.
Not now: Drain (C10), MCP wiring.

### C10. Drain at points (the auto search, channels and plugs are C10b)
Read: `crates/encrust-cli/src/hollowing.rs` (`hole_near`, `refuse_off_a_face`, `OFF_A_FACE_DEG`,
  `ON_THE_SURFACE_MM`), `crates/core-volume/src/{drain.rs,model.rs}` (`HoleSize`, `DrainHole`,
  `HoleKind`, `ModelHollow::add_hole`, `restored`), `crates/core-engine/src/project/restore.rs`
  (`hollow_of`), `crates/core-document/src/{apply.rs,document.rs,hollow.rs}`, `crates/encrust-app/src/drain.rs`
Touch: those, `crates/core-edit/src/edit.rs` (+tests), new `crates/core-document/src/drain.rs`
  (+ `drain/tests.rs`), `crates/core-document/src/{lib.rs,error.rs}`, `crates/encrust-app/src/link/plate.rs`
  (+ test), `crates/core-volume/src/drain.rs` (serde derives on `HoleSize` only), `docs/design/document.md`,
  `docs/cli.md` and `docs/features.md` (name the edit)
Edit: `Drain { object, at_mm: Vec<Vec3>, size: HoleSize, kind: HoleKind }` (`size` defaults to
  `HoleSize::default()`, `kind` to a drain) — a hole drilled into the surface nearest each plate
  point, as `--drain-at` drills one.
Move: `hole_near` + `refuse_off_a_face` and their two constants → core-document/src/drain.rs as
  `hole_near(mesh, bvh, near_and_up, size, kind) -> Result<DrainHole, ApplyError>`; the CLI keeps
  only its message for the error. `apply`: each point taken into the object's own space (inverse
  of its transform), `hole_near` on its source mesh, then `ModelHollow::add_hole` on `hollow_of`
  its state, and the drains and channels written back to `ObjectHollowState`; the shell is not
  rebuilt (the holes are cut at bake, as a project file keeps them).
Errors: no surface near a point → `ApplyError::NoSurface(ObjectId)` (exists); a point off an
  edge or corner → new `ApplyError::OffAFace { object, at_mm: Vec3 }`.
Window: the link's Drain arm (a helper) runs the same `hole_near` on `object.mesh` and
  `object.hollow.add_hole`; the window's drilling job cuts them on its next frame. Nothing under way.
Done when: a 10 mm cube drained at (5,5,10.5) (over its top face) gets one hole whose mouth is
  at (5,5,10) ± 1e-3 in its own space and whose axis points down (−Z ± 1e-4); a point at the
  corner (10.5,10.5,10.5) → `OffAFace`; on a model with no faces → `NoSurface`; the plate written
  and read back keeps the hole. Window link test of `Drain`. Existing `--drain-at` tests pass.
Not now: the auto search, channels, plugs, holes into the supports (C10b), MCP wiring.

### C11. Supports — blocked on a decision (not dispatched)
The window grows supports by its own job (`job/place.rs`: `generate_supports`, `strengthen`)
and keeps the trees; `--supports` and the autopilot stand them by `core_engine::stand_under`.
A `.encrust` holds only grown trees (`ObjectSupportState::grown`), which `open_plate` reads and
never decides again. So `Edit::Supports` cannot be one `apply` until the two ways are one: an
ADR choosing which grows the trees for both (window wins by ADR 0466, but `stand_under` is
the engine's, ADR 0402), then the job moved down. Until then a client's supports reach the
window by the old recipe path. Same for the drains' auto search and plugs (C10b).

## Status (2026-11-19)
Done and pushed: M0, M1, C1–C7, C8 (decided: a pose is a Rotate), C9, C10, and the MCP side of
every edit (`server/asked.rs`: a revision's changes beyond the one before go to a followed
window as edits; what no edit carries keeps the recipe's path).
Open, each needing a decision or a design session before it is a card:
- C11 Supports: one way of growing them for the window and the engine (see C11).
- C10b: the engine's drain search, plugs, channels, holes into the supports.
- C2b Arrange: the plate's area and the supports' footprints in the Document.
- C12: recipes yield edits, both recipe executors deleted.
- C13: a revision as a base `.encrust` and its edits (`--project-out` for a model input, a
  Document staged from models); then `unshown.rs` and the flags path go, and an undo of any
  edit (not only a cut) reaches the client.
- C14: 0459's status, `ai-integration.md`, snapshots if a panel changes.

## Decided 2026-11-19: supports the window's way (ADR 0468); a revision is a file and its edits (ADR 0466)

### C13a. The window's first numbers move into the core
Read: `crates/encrust-app/src/{hollow.rs,drain.rs,supports.rs,slicing.rs,tool_settings.rs}` (each
  tool's `Default`, `support_state`, `slicing_state`), `crates/encrust-app/src/cut.rs` (`CutTool`
  default), `crates/core-engine/src/project.rs` (`Manifest` and its `*State` types)
Touch: those, `crates/core-engine/src/project.rs` (or a new `project/defaults.rs` if it passes
  ~400 code lines), `docs/formats/encrust-project.md` if it names defaults
Move: the numbers a fresh window's tools start at become `Default` impls of core-engine's
  `HollowState` (from `HollowSettings::default()`, infill off, floor closed), `DrainState` (the
  engine's `HoleSize::default()`), `CutState` (Z, 10 mm, both, no joint, default parts),
  `SlicingState` (islands kept, no hanging passes, the default format), `SupportState` (one
  group named as the window's `FIRST_GROUP` on `SupportProfile::default()`, the window's brush,
  flood angle, inline spacing and fill numbers), and `Manifest::new(printer: Option<String>,
  resin: Option<String>)` — version 7, those states, one plate named as the window names its
  first, no objects. The window's tool defaults then read their state from these (`state:
  HollowState::default()` and so on), so a number lives once.
Done when: a test in core-engine that `Manifest::new(..)` written and read back is equal in
  every field; the window's `Tools::default()` captured (`ToolSettings::of`) equals the
  defaults field by field (one test in encrust-app); all existing tests pass unchanged.
Not now: the command line writing a project (C13b).

### C13b. `--project-out` takes models
Read: `crates/encrust-cli/src/{plate_edits.rs,stage.rs (stage, models, plate_of)}`,
  `crates/core-document/src/{document.rs,piece.rs}`, C13a's `Manifest::new`
Touch: those, `crates/encrust-cli/tests/cli/plate_edits.rs`, `docs/cli.md`, `docs/features.md`
Decision: a model input with `--project-out` and none of the flags that shape, hollow, drain or
  support it (the ones `project.rs::refuse_shaping_flags` lists, plus `--cut`, `--split`,
  `--part-off`, `--combine`, `--label`, `--prepare`, `--recipe`) writes a project of the models
  as imported and placed: `Manifest::new(printer id, resin id)` and one object each
  (`Document::of_models`, new: name from the file stem, source mesh, identity transform —
  the mesh is placed already —, the import's diagnostics as its summary, ids from 1). With such
  a flag it is refused ("--project-out writes models as imported; shape them by --edits on the
  project it writes"). `--edits` with models is then allowed only together with
  `--project-out`, applied to that document before it is written and staged as a project.
Done when: `estimate cube.stl --project-out a.encrust` writes one object, id 1, its source the
  cube's 8000 mm³ ± 0.01; `--edits` with a cut on it writes two halves; `--hollow` with
  `--project-out` on a model is refused; existing tests pass.
Not now: MCP using it (C13c, orchestrator).

### Supports the window's way (ADR 0468) — mapped, not started
Two needs paths and two growth paths today:
- needs — the window: `encrust-app/src/job/place.rs::needs_of` (+`plan_of`): sliced where it
  stands, `generate_supports` fed painted/blocked/shown faces, seeds and `low_points`, then
  `strengthen`; the engine: `core-engine/src/stand.rs::needs_under` (no paint, no seeds, its
  own lift in `touched`).
- growth — the window: the placements land as `ModelSupports` points
  (`encrust-app/src/supports/run.rs::land_placed`, with groups, frozen trees, a cavity's inside
  and regrowing kept), then `ModelSupports::growth(..).grow(model, surface, bvh)`
  (`core-supports/src/model/growth.rs`) on the window's growth job; the engine:
  `core-engine/src/stand.rs::grown` → `core_supports::branch::grow(columns, placed, profiles)`.
Cards, in order, each green on its own:
- S1: `needs_of` + `plan_of` move into core-engine (`stand/needs.rs`), taking the window's
  inputs as options; the window's job calls it; `needs_under` becomes it with none given. CLI and
  support-lab numbers may move: run `support-lab` before and after, keep the new baseline.
- S2: the engine grows its trees the window's way: from needs to `ModelSupports` points to
  `growth().grow`, replacing `branch::grow` in `grown`; the same bench before and after.
- S3: `Edit::Supports { object, profile, at_mm, only_at }` in core-document, its trees into
  `ObjectSupportState`; the window link's arm starts the support tool's job for that object.
- S4: MCP: `asked.rs` hands supports over; docs and the `support-lab` baseline commit.
Best started in a fresh session: each card needs the window's support tests read closely.

### S1. One way to find where a model needs holding up (no behaviour change)
Read: `crates/core-engine/src/stand.rs` (`needs_under`, `Run`, `Run::place`, `Run::strengthen`,
  `shown_on`), `crates/encrust-app/src/job/place.rs` (`SupportTask`, `needs_of`, `plan_of`),
  `crates/core-supports` (`generate_supports`, `Blocked`, `Shown`, `WallSide`, `Need`),
  `crates/core-engine/src/strengthen.rs`
Touch: those two files, new `crates/core-engine/src/stand/needs.rs` (+ its tests if any), `crates/core-engine/src/lib.rs`
Move: `Run::place` + `Run::strengthen` and the window's `needs_of` body become one public
  core-engine function in `stand/needs.rs`, roughly
  `needs_on(asked: &NeedsAsked<'_>, held: &[Vec3], tick: &mut dyn FnMut(usize) -> bool) -> Result<Vec<Need>, EngineError>`
  where `NeedsAsked { stack, surface (placed), bvh, layer_height_mm, plan: &SupportPlan,
  lows: &[Vec3], painted: Option<Blocked>, shown: Option<&Shown>, seeds: &[Vec3],
  side: Option<WallSide> }`: `generate_supports` with `Blocked::keeping(painted, side)`, the
  seeds kept by the side, the needs kept by the side, then `strengthen` with `held` standing.
  `tick(layer)` answers whether to go on (the window's cancel and progress; the engine passes
  `|_| true`). `plan_of` moves beside it as `SupportPlan::sized(profile, sizing)` (or a free fn).
  `needs_under` calls `needs_on` for the outside and the inside as it does now; the window's
  `needs_of` builds the stack, the painted `Blocked`, `Shown` and seeds as now and calls it.
Done when: nothing changes — every existing test passes unchanged: `cargo test -p core-engine`,
  `cargo test -p encrust-app --lib job::place` and `--lib supports`, `cargo test -p encrust-cli
  --test cli support`. No test is edited. `generate_supports` and `strengthen` are each called
  from one place in core-engine and nowhere in encrust-app (`git grep`).
Not now: growth (S2), the edit (S3).

### S2 — folded into S3
The two growth paths are one at the bottom (points → `columns` → `grow_beside`/`grow`); the
window's adds its painted region kept clear and its frozen trees. S3 grows through the window's
`ModelSupports::growth(..).grow(..)`, which is that.

### S3. Supports by an edit
Read: `crates/core-engine/src/stand/needs.rs` (S1's `needs_on`, `plan_of`), `crates/core-engine/src/project/restore.rs`
  (`supports_of`), `crates/core-supports/src/model/{points.rs,growth.rs,build.rs}` (`add`, `growth`,
  `Growth::grow`, `take_grown`, `grown`), `crates/encrust-app/src/job/place.rs` (how the window
  builds a task: stack from the placed printed surface, `Blocked`, `Shown`, seeds),
  `crates/encrust-app/src/supports/run.rs` (`land_placed`, `add_needs`), `crates/core-document/src/{apply.rs,hollow.rs,drain.rs}`
Touch: those you need, `crates/core-edit/src/edit.rs` (+tests), new `crates/core-document/src/supports.rs`
  (+ `supports/tests.rs`), `crates/core-document/src/{lib.rs,error.rs}`, `crates/encrust-app/src/link/plate.rs`
  (+ test), `docs/design/document.md`, `docs/cli.md` and `docs/features.md` (name the edit)
Edit: `Supports { object, profile: Option<String>, at_mm: Vec<Vec3>, only_at: bool }`.
  `profile` is a shipped preset's name (`SupportProfile::shipped`), the plate's active group's
  profile when `None`; `auto` is not taken yet (`ApplyError::NoProfile` names it; the machine's
  sizing is the window's catalogue, not the document's). `at_mm` are contacts placed by hand
  (the closest surface point of each); `only_at` places those alone.
apply: the object's `ModelSupports` restored by `supports_of`; needs by `needs_on` on its
  printed surface placed on the plate, its blocked and shown regions and its present contacts as
  seeds, the outside of a hollow wall (`WallSide` as the window's job builds it), with
  `plan_of(profile, None)`; each need and each `at_mm` point `ModelSupports::add`ed to the active
  group; grown by `growth(..).grow(..)` and `take_grown`; the points and `grown(transform)`
  written back to `ObjectSupportState`. The plate's group table gains the profile as a group
  where it names one not in it.
Window: the link's Supports arm (a helper) adds the `at_mm` points and, unless `only_at`, starts
  the support tool's automatic run for that object alone (as the panel's Auto does), under way
  until it lands; the window's growth job grows the trees as for a click.
Errors: an unknown profile → `NoProfile(String)`; `Cancelled`; others `Failed`.
Done when: a 10 mm cube lifted 5 mm (translation z 5) supported with `medium` gets supports
  (grown trees non-empty, every tip's contact on the cube's underside z 5 ± 1e-3 in plate mm);
  the same cube standing on the plate gets none; `only_at` with one point at (5,5,4) gets exactly
  one point at (5,5,5) ± 1e-3; `profile: "auto"` → `NoProfile`; the plate written and read back
  keeps the trees. Window link test of `Supports` with `only_at`. Existing tests pass.
Not now: `auto` sizing, the cavity's inside, merges, shown-at strokes, MCP (S4).

## Status (2026-11-19, later)
Done: C13a–c (with a followed window a revision of edits is checked on its plate and kept as
`N.encrust` + `N.edits.json`); S1, S3, S4 (supports the window's way, by a named preset).
Open: supports sized `auto` as an edit (needs the machine's sizing in the Document), a
cavity's inside, merges, shown-at strokes; C10b; C2b; C12; C14; `Only.sized` in
encrust-app/src/supports/start.rs is always false now and can go.

## Round 3 (2026-11-19): items 1–8 of the remaining list

### R8. A hollow at precision 1.0 keeps far too much
Fact: a 20 mm cube hollowed with a 2 mm internal wall (core-document's `shelled`, the window's
  path: `cavity_field` + `carve`) keeps 4058 mm³ at precision 0.5, 3981 at 0.75, ~3962 at 0.85
  and 0.9, but 5575 at 1.0 (closed form 20³ − 16³ = 3904). Something goes wrong at the fine end.
Read: `crates/core-volume/src/hollow.rs` (`lattice_mm`, `COARSE_LATTICE_MM`, `FINE_LATTICE_MM`,
  the surface cap of ADR 0083), the field and carve code it calls, `crates/core-document/src/hollow/tests.rs`
Do: find why (a cap that coarsens instead, a lattice that no longer fits the wall, a cavity
  dropped piece by piece…), with a failing test first in core-volume: the 20 mm cube at
  precision 1.0 keeps 3904 within the lattice's error (state it). Fix it in core-volume. If the
  cause is a deliberate rule (ADR 0083) and 5575 is what the rule means, stop and report with
  the numbers instead of changing it.
Done when: that test passes, the existing hollow tests pass, core-document's hollow test can use
  precision 1.0 with a tighter tolerance (change its comment and numbers).

### S5. Supports sized automatically, under a cavity, off what shows (items 1 and 3)
Read: `crates/core-document/src/supports.rs` (+tests), `crates/core-engine/src/stand.rs`
  (`SupportPlan::sized`, `SupportShapes::Sized`, `plan.cavity`, `plan.shown_at`, `ShownAt`),
  `crates/core-engine/src/stand/needs.rs`, `crates/encrust-cli/src/profiles.rs` (`plan_of`,
  `auto_plan`), `crates/printer-profiles/src/{auto.rs,catalogue.rs}`, `crates/core-engine/src/project.rs`
  (`SupportState::sizing`, `AutoSizing`), `crates/encrust-app/src/link/plate.rs` (the Supports arm),
  `crates/encrust-app/src/supports/start.rs`
Touch: those, `crates/core-edit/src/edit.rs` (+tests), `docs/design/document.md`, `docs/cli.md`
Edit: `Supports` gains `cavity: bool` (the inside of a hollow wall held up too, as `--support-cavity`)
  and `shown_at: Vec<ShownAt>` (strokes of the side meant to be seen, as the plan's), both
  defaulting off/empty; `profile: Some("auto")` is now taken.
`auto`: the plate's own automatic profile where its `SupportState::sizing` keeps one, else the
  shipped catalogue's first, as `--supports auto` takes the catalogue's; built by
  `SupportPlan::sized` against the shipped catalogue (`Catalogue` with no user folder — find
  how printer-profiles builds it). The window's link arm for `auto` sizes the run as the panel's
  Auto does (`sized: true` for it alone; a named preset stays `sized: false`).
cavity: the inside side placed and strengthened as `needs_under` does it (S1's `placed_on` /
  `strengthened_on`), the inside's needs in the same group; the window arm runs the panel's
  inside pass for the object.
shown_at: the plan's `shown_at`, so `needs_on` takes the shown faces from the strokes as the
  engine does where the object has no painted shown region; the painted one wins where it has.
Done when: `auto` on the 10 mm cube lifted 5 mm grows trees (tips > 0, contacts on z 5 ± 1e-3);
  a 20 mm cube hollowed with a 2 mm wall and lifted, with `cavity`, has contacts inside the cavity
  (some contact's z above 7 ± 1e-3, the cavity's ceiling at 5 + 18); a stroke over the underside's
  middle leaves fewer contacts within its radius than without it (the counts stated); a window
  link test of `auto`. Existing tests pass; `NoProfile("auto")`'s test becomes one for an unknown
  name.
Not now: merges (no client asks them), MCP (orchestrator).

### D2. The engine's drain search, channels and plugs by edits (item 2)
Read: `crates/core-engine/src/drains.rs` (`DrainSearch`, `suggest_drains_heard`, `Drained`,
  `HoleRole`), `crates/encrust-cli/src/hollowing/auto.rs` and `hollowing.rs` (`placed` for
  `--channel`, `plugs_beside`, `cut_plugs`), `crates/encrust-app/src/job/{drains.rs,plugs.rs}`,
  `crates/encrust-app/src/drain.rs` (`drill_found`, `finish_channels`) and `drain/plugs.rs`,
  `crates/core-document/src/{drain.rs,apply.rs}`, `crates/core-volume` (`Channel`, plugs)
Touch: those you need, `crates/core-edit/src/edit.rs` (+tests), `crates/core-document/src/drain.rs`
  (split into `drain/` files if it passes ~400 code lines), `crates/core-document/src/{lib.rs,error.rs}`,
  `crates/encrust-app/src/link/plate.rs` (+ test), `docs/design/document.md`, `docs/cli.md`
Edits (each one moved computation, no copy):
- `Drain` gains `auto: bool` and `vent: bool`: with `auto`, the engine's search
  (`suggest_drains_heard`, cancel through the `Cancel`) on the object as it stands, the holes it
  proves added to the object's hollow, a vent per pocket where `vent`. The CLI's `--drain-at auto`
  and the window's search land through the same core-document function.
- `Channel { object, points_mm: Vec<Vec3>, diameter_mm }`: a drainage channel along the plate
  points, as `--channel` lays one; added to the object's hollow channels.
- `Plugs { object, gap_mm }`: a plug for each of the object's drain holes, laid beside it as
  new objects (`plug_of` naming it), as `--drain-plugs` and the window's Drain tool lay them;
  `Role::Plug(n)`.
Window: the link's arms (helpers) run the window's own search job / channel / plugs job for the
  object, under way until each lands.
Errors: `Cancelled`; no cavity to drain → `ApplyError::NoCavity(ObjectId)`; others `Failed`.
Done when: a 20 mm cube hollowed with a 2 mm wall, `Drain { auto }` gets at least one hole whose
  mouth is on the cube's surface (± 1e-3) and no resin left trapped (`trapped_in` reports none);
  a solid cube → `NoCavity`; `Channel` adds one channel of the points given; `Plugs` on a cube
  with two holes adds two plug objects naming it; window link tests of each. Existing tests pass.
Not now: holes into the supports (`support_at_mm`), MCP (orchestrator).

### A1. Arrange by an edit (item 4)
Read: `crates/encrust-app/src/arrange.rs` (+ `arrange/turn.rs`), `crates/core-plate/src/arrange.rs`
  (`arrange`, `Footprint`, `PlateArea`, `ArrangeSettings`), `crates/encrust-cli/src/stage/room.rs`
  (`spread_pieces`), `crates/core-document/src/{apply.rs,plate_contents.rs}`
Touch: those you need, `crates/core-edit/src/edit.rs` (+tests), new `crates/core-document/src/arrange.rs`
  (+ tests), `crates/core-document/src/lib.rs`, `crates/encrust-app/src/link/plate.rs` (+ test),
  `docs/design/document.md`, `docs/cli.md`
Edit: `Arrange { plate_mm: [Scalar; 2], clearance_mm: Scalar }` — every printable object of the
  active plate spread over a plate of that size, each with its supports' footprint, turned the
  least where no slide fits, as the window's Arrange does; the window's footprint and pack steps
  move into core-document (`arrange_plate`'s computation, not its `Status`), the window keeps
  only its message. `Report::Arranged { placed, left_out }`.
Done when: four 10 mm cubes stacked at the origin, arranged on a 100 × 100 plate with 2 mm
  clearance, leave no two footprints closer than 2 mm (± 1e-3) and all inside the plate; five
  50 mm cubes on that plate leave some out (`left_out` named); a window link test. Existing
  arrange tests pass.
Not now: MCP (orchestrator).

### R6. Recipes yield edits (item 6) — four cards
A recipe's steps: Repair, Orient, Hollow, Drain, Supports, Arrange change the plate; Check,
Save, Gate, Pause steer the run. The executors stay for the latter (conditions, measuring,
writing); each plate step becomes one edit carried out by `apply` (CLI) or the window's link
machinery (window).
- R6a `Edit::Orient` (card below; C8 revised, ADR 0470).
- R6b core-recipe's `edits_of` (card below).
- R6c the window's Prepare for print (`prepare/{work,step}.rs`) runs each plate step through
  `edits_of` and the link's `start_edit` path, its own per-step job calls deleted; deferred,
  it waits on batched edits. Its Supports edits set `clean_surface` from the Orient tool's
  Clean surface (prepare/step.rs:83 reads it today).
- R6d the CLI's recipe executor (`recipe/{steps,plate,piece}.rs`) runs on a `Document` of the
  staged models through `apply`; this is the command line's model pipeline moving onto the
  document (the original plan's phase 5) and needs its own design pass first.

### R6a. Orient by an edit
Read: `crates/core-engine/src/trial.rs` (`orient_for_print`, `Choice`), `crates/encrust-cli/src/pipeline/place.rs`
  (`orientation`, `orient_settings`, `orient_preset`), `crates/encrust-app/src/job/orient.rs`
  (`OrientTask::of`, the `Printed` it builds), `crates/core-recipe/src/orient.rs` (`preset_of`),
  `crates/core-document/src/{apply.rs,placement.rs,supports.rs}`, `crates/core-edit/src/edit.rs`
Touch: those you need, new `crates/core-document/src/orient.rs` (+ tests), `crates/core-document/src/{lib.rs,error.rs}`,
  `crates/encrust-app/src/link/plate.rs` or a new `link/plate/orient.rs` (+ test), `docs/design/document.md`, `docs/cli.md`
Edit: `Orient { object, goal: OrientAim, max_tilt_deg: Option<Scalar>, clean_surface: bool, pick: usize }`,
  `OrientAim` a core-edit enum `auto | fewest-supports | least-peel | shortest-print` (kebab serde,
  default auto) mapped to `OrientPreset` in core-document; `preset_of` then maps through it, one
  table of the words.
apply: `orient_for_print` with no trial on the object as it stands (outer, shell, cuts, drain vents,
  what shows — the `Printed` the window builds, built once in core-document and called by the window's
  `OrientTask` where it can be), settings from the object's support plan as `orient_settings` makes
  them; the `pick`th choice (past the last → `ApplyError::NoChoice { object, pick, found }`), its
  rotation set as `Rotate` sets one and the model dropped onto the plate as Rotate does.
  `Report::Oriented { rotate_deg }`. Cancel through `Cancel` → `Cancelled`.
Window: the link's arm runs the window's own orient job for the object and picks the `pick`th pose
  it offers, under way until it lands.
Done when: a 10 × 10 × 30 mm cuboid lying on its side, `Orient { goal: shortest-print }` stands it
  with its 30 mm along the plate (height ≤ 10 + 1e-3); `pick` past the choices → `NoChoice`; a
  cancelled search → `Cancelled`; a window link test; existing tests pass. `docs/design/document.md`
  lists Orient as an edit again with one line saying a recipe's orient step has no check to choose
  for it (C8 revised).
Not now: a machine's trial, keep-clear points, MCP (orchestrator), the recipe (R6b).

### R6b. A recipe's plate step is an edit
Read: `crates/encrust-app/src/prepare/work.rs` (`Work::of`, `Hollowed::of`, `drain_of`, `method_of`, `cavity`),
  `crates/encrust-cli/src/recipe/steps.rs` and `recipe/plate.rs` (how the CLI reads the same keys),
  `crates/core-recipe/src/{lib.rs,resolved.rs,recipe.rs}`, `crates/core-edit/src/edit.rs`
Touch: new `crates/core-recipe/src/edits.rs` (+ tests), `crates/core-recipe/src/lib.rs`,
  `crates/core-recipe/Cargo.toml` (+ core-edit), `Cargo.lock`, `docs/architecture.md` (the edge),
  `crates/encrust-app/src/prepare/work.rs` (its key reading replaced by the new fn where it maps one to one)
Do: `edits_of(kind: StepKind, settings: &ResolvedSettings, object: ObjectId, placed: Aabb, plate_mm: [Scalar; 2])
  -> Result<Vec<Edit>, NotAnEdit>` for the six plate steps: Repair → `Repair`; Orient → `Orient`
  (R6a; goal word through `OrientAim`, `pick` 0); Hollow → `Hollow` (wall, mode, open floor, infill,
  solid-under as the window's `Hollowed` reads them); Drain → `Drain` (`at = auto` → `auto`, `vent`;
  `at = points` → the points as fractions of `placed` turned into plate mm) then `Plugs` when `plugs`;
  Supports → `Supports` (profile word, `cavity`); Arrange → `Arrange { plate_mm, clearance_mm: spacing_mm }`.
  Defaults as the window's `Work::of` takes them today (`PREPARE_ORIENT`, `PREPARE_REPAIR`, …),
  moved, not copied. `NotAnEdit(StepKind)` for Check, Save, Gate, Pause and for Arrange with
  `only_in_the_way` (the window's slide-clear, no edit yet).
Window: `Work::of` reads its numbers through `edits_of` where a step maps one to one; nothing about
  how it runs changes yet (R6c).
Done when: one test per plate step that its settings land in its edit (the numbers given in, the
  same numbers out), Drain points: a fraction (0.5, 0.5, 1.0) of a box 0..10 lands at (5, 5, 10)
  ± 1e-6; Check → `NotAnEdit`; existing core-recipe and encrust-app prepare tests pass.
Not now: running the prepare on edits (R6c), the CLI executor (R6d).

## Status (2026-11-19, round 3 end)
Done: R8, S5, item 5, D2 (C10b but holes into the supports), A1 (C2b), R6a, R6b (ADR 0469, 0470),
C14's docs. Deferred by the user: R6c — the window's prepare runs one job over many models
where the link's edits run one model at a time, so it waits on batched edits (a design
session); R6d — the CLI's recipe executor on a `Document`, its own design pass.

## Round 4 (2026-11-19): one result whatever the front end

Decided by the user: **the window is the reference**; the edit path (`apply`, the link, MCP) and
the CLI do what it does. The document and the edits know nothing of a printer or resin: model
analysis runs at one fixed step; a plate area and a plug gap come into an edit as final numbers,
the front end that knows the machine (window, CLI, MCP) working them out. Where the window is
itself inconsistent, the panel button is the reference. Run order: F → B → P → D1 → R6d.
Checked, no card: 21 (`core-recipe/src/recipe.rs:91` refuses `close-small-holes` and `turn`, so
`edits.rs:78-85` never sees them); 10 is the CLI's own counting (D1 keeps or drops it).

### F1. A hole drilled by an edit on a scaled object prints at its size (bug 14)
Fact: `hole_near` (core-document/src/drain.rs:32) drills on the own-space mesh with the size in
  plate mm, unscaled; `apply_drain` (drain.rs:126) and the link (link/plate/drain.rs:32) call it,
  so on an object at scale 2 a 3 mm hole prints 6 mm. The window's click goes through
  `ModelHollow::drain_at` (core-volume/src/model.rs:376), which scales through `into_model`.
Read: `crates/core-document/src/drain.rs` (+ `drain/tests.rs`), `crates/core-volume/src/model.rs`
  (`drain_at`, `in_model`), `crates/encrust-app/src/drain/points.rs`, `crates/encrust-app/src/link/plate/drain.rs`,
  `crates/encrust-cli/src/recipe/drain.rs` (`at_points`, the other caller)
Touch: those, `crates/encrust-app/src/link/tests/` (the drain test)
Do: failing test first. `hole_near` takes the plate point and the object's transform and builds
  the hole as the click does: closest own-space point, its face's plate normal, `drain_at`; the
  `OffAFace` check stays, in plate mm. The CLI passes `Transform::default()` (its mesh is baked).
Done when: a 10 mm cube scaled 2, `Drain` at its top middle (10, 10, 20) with a 3 mm hole: the hole
  measured on the plate (own diameter × 2) is 3 ± 1e-4 (6 before); the same through a window link
  test; existing drain tests pass.
Not now: the hole's kind (P7).

### F2. Drilling takes away the supports it drills out, every path (bug 18)
Fact: the window's `SceneObject::drill_hole` (encrust-app/src/scene/support_surface.rs:155-165)
  clears every contact the new hole drills out (`drilled_out`, ADR 0443); `apply_drain`
  (drain.rs:138, 147), the link (link/plate/drain.rs:37) and Check's Drill them
  (`DrainTool::drill_vents`, encrust-app/src/drain.rs:177, `add_drain`) only add the hole.
Read: `crates/encrust-app/src/scene/support_surface.rs`, `crates/core-volume/src/drain.rs`
  (`drilled_out`), `crates/core-supports/src/model/points.rs` (`clear_contacts`), `crates/core-document/src/drain.rs`,
  `crates/core-engine/src/project.rs` (`supports_of`, how core-document stores a model's supports),
  `crates/encrust-app/src/link/plate/drain.rs`, `crates/encrust-app/src/drain.rs` (`drill_vents`)
Touch: those, `crates/core-document/src/drain/tests.rs`, `crates/encrust-app/src/link/tests/`
Do: failing tests first. `apply_drain` clears, after each hole it adds, the object's contacts that
  hole drills out (one helper beside `hole_near`, the clearing in core-volume/core-supports as is);
  the link and `drill_vents` call `drill_hole` instead of `add_hole`/`add_drain`.
Done when: a 10 mm cube lifted 5 mm with contacts at (5, 5, 5) and (1, 1, 5), `Drain` of 3 mm at
  (5, 5, 5): the first is gone, the second stays (contact count 2 → 1); the same by a link test and
  by `drill_vents` over a vent at (5, 5, 5); existing tests pass.
Not now: holes into the supports (`support_at_mm`).

### F3. A recipe's Check drills cups at the recipe's hole size (bug 20)
Fact: the prepare's Check (encrust-app/src/prepare/check.rs:154) calls `drill_cups`, which sizes
  vents by the Drain panel (`drain.size()`, check.rs:175, drain.rs:165) — against
  prepare/work.rs:1-2 ("never what a panel holds"). The CLI takes the piece's hole size
  (encrust-cli/src/recipe/plate.rs:127-130).
Read: `crates/encrust-app/src/check.rs` (`drill_cups`, `cup_owners`), `crates/encrust-app/src/drain.rs`
  (`drill_vents`), `crates/encrust-app/src/prepare/{check.rs,work.rs}`, `crates/encrust-app/src/panels/inspector/check/fixes.rs`
Touch: those, their tests
Do: failing test first. `drill_cups` and `drill_vents` take the `HoleSize`; the panel's Drill them
  passes `drain.size()`, the prepare passes its run's drain step size (`Work`'s drain `HoleSize`),
  else `HoleSize::default()`.
Done when: a prepare whose drain step sets `diameter_mm = 2.5` with the panel at 4 drills cup vents
  of 2.5 ± 1e-4; with no drain step, `HoleSize::default().diameter_mm`; the panel's button still
  takes the panel's; existing check tests pass.
Not now: anything about levers.

### B1a. One analysis step: the constant, the bench, the ADR (items 1, 17)
Fact: supports and the drain search slice the model; the edit path uses `SliceSettings::default()`
  (0.05 mm: core-document/src/supports.rs:324, drain/auto.rs:70), the window and CLI the machine's
  layer height. Step chosen: **0.05 mm** — already the edit path's, the slicer's default and the
  height every shipped resin is measured at (`printer-profiles`: all `layer_height_mm = 0.05`);
  supports read layers onto 0.1 mm cells (`docs/design/supports.md`), so a finer step buys nothing
  there. The user may overrule from the bench.
Read: `crates/core-engine/src/{stand.rs,stand/needs.rs,drains.rs}` (`needs_on`, `DrainSearch`),
  `crates/core-engine/benches/plate.rs`, `crates/core-document/src/{supports.rs,drain/auto.rs}`,
  `docs/decisions/0000-template.md`, `docs/design/{supports.md,hollowing.md}` (the two "at the run's
  layer height" sentences)
Touch: `crates/core-engine/src/lib.rs` (+ the file owning the constant), `crates/core-engine/benches/plate.rs`,
  `crates/core-document/src/{supports.rs,drain/auto.rs}`, new `docs/decisions/0471-model-analysis-at-one-step.md`,
  `docs/decisions/README.md`, `docs/design/{supports.md,hollowing.md,document.md}`
Do: `core_engine::ANALYSIS_LAYER_MM: Scalar = 0.05` with one line of rustdoc; core-document reads
  it. Bench first: a criterion group timing `needs_on` and `suggest_drains_heard` on a 20 mm cube
  hollowed to a 2 mm wall at 0.02, 0.05 and 0.1 mm (the shipped range: before = the machine's
  step, after = the constant). ADR 0471 draft, one record for the three decisions taken together:
  analysis at one step (the bench's numbers), a plate area passed into an edit as the usable area
  (B2), a plug gap in an edit final (B3). The design pages say "at the analysis step" instead.
Done when: the bench runs and its numbers are in the ADR; core-document's supports and drain tests
  pass unchanged (0.05 = the old default); no `SliceSettings::default().layer_height` left in
  core-document.
Not now: the window and CLI call sites (B1b); slicing, Check's measure and the orient trial keep
  the machine's height (they judge the print, not the model).

### B1b. The window and the CLI analyse at that step
Read: B1a's constant; the call sites: `crates/encrust-app/src/panels/inspector/{supports.rs:504,drain.rs:138}`,
  `panels/inspector/check/plate.rs:298`, `prepare/step.rs:59,150`, `link/plate.rs:490`,
  `link/plate/drain.rs:41`, `app/showcase.rs:214`; `crates/encrust-cli/src/stage.rs:458`,
  `stage/ground.rs:20`, `recipe/drain.rs:118`, `recipe/piece.rs` (`trapped`, `ctx.layer_height_mm`)
Touch: those files only, one test each side
Do: each supports run, drain search and trapped-resin scan passes `ANALYSIS_LAYER_MM` instead of
  `slicing.layer_height_mm()` / the CLI's layer height. Sites that slice the print (section view,
  plate summary, Check's measure, file names) are left alone.
Done when: a window test with the machine at 0.1 mm: the supports job and the drain job are asked
  for 0.05 ± 1e-9; a CLI test: `--supports` on a cube gives the same contacts at `--layer-height
  0.1` and `0.03` (counts equal); existing tests pass.
Not now: the CLI's lift before drains (R6d).

### B2. Arrange takes the usable area (item 11)
Fact: the window's Arrange packs into `BuildPlate::plate_area()` (plate minus `PlateInset`,
  encrust-app/src/plate.rs:84, arrange.rs:45); the link rebuilds a plate with no inset
  (link/plate/arrange.rs:20-25), the edit packs `PlateArea::whole` (core-document/src/arrange.rs:154).
  An inset is asymmetric, so a size alone is not enough.
Read: `crates/core-edit/src/edit.rs` (+ tests), `crates/core-document/src/arrange.rs` (+ tests),
  `crates/core-plate/src/arrange.rs` (`PlateArea`), `crates/encrust-app/src/link/plate/arrange.rs`,
  `crates/encrust-app/src/plate.rs`, `crates/core-recipe/src/edits.rs` (+ tests), `crates/encrust-app/src/prepare/work.rs`
Touch: those, `docs/design/document.md`, `docs/cli.md`
Edit: `Arrange` gains `#[serde(default)] low_mm: [Scalar; 2]`; `plate_mm` is the usable area's size
  from it (`PlateArea { low_mm, high_mm: low_mm + plate_mm }`). `edits_of` takes the area as
  `(low_mm, plate_mm)`; the window's prepare passes `plate_area()`; the link builds the area from the
  edit. The same pair is what P5 and P6 add.
Done when: two 10 mm cubes, `Arrange { plate_mm: [50, 50], low_mm: [10, 20], clearance_mm: 2 }`:
  every footprint within x 10..60, y 20..70 (± 1e-3); an edit without `low_mm` reads as today; a
  link test with an inset of 5 on each side keeps every model 5 mm in; existing arrange tests pass.
Not now: the CLI's area (R6d), MCP (orchestrator).

### B3. A plug gap in an edit is the final gap (item 19, gap)
Fact: `apply_plugs` cuts at `gap_mm / scale` (core-document/src/drain/plugs.rs:52): already the
  plate gap, no drift. The link sets `plug_gap_mm = gap` and `lay_plugs` takes the resin's drift off
  again (link/plate/drain.rs:73-75, drain/plugs.rs:314); `edits_of` writes `PLUG_GAP_MM` raw
  (core-recipe/src/edits.rs:132).
Read: `crates/core-document/src/drain/plugs.rs`, `crates/encrust-app/src/link/plate/drain.rs`,
  `crates/encrust-app/src/drain/plugs.rs` (`plugs_for`), `crates/core-recipe/src/edits.rs` (+ tests),
  `crates/encrust-app/src/prepare/work.rs`, `crates/core-edit/src/edit.rs` (`Plugs` rustdoc)
Touch: those, `crates/encrust-app/src/link/tests/`, `docs/design/document.md`
Do: the link stores `plug_gap_mm = gap_mm + drift_mm`, so the tool's subtraction leaves the edit's
  gap; `edits_of` takes `plug_drift_mm` and writes `PLUG_GAP_MM - plug_drift_mm`, the window's
  prepare passing its resin's `fit_drift_mm()`. `Plugs`' rustdoc says the gap is final, plate mm.
Done when: a link test with a resin of drift 0.05, `Plugs { gap_mm: 0.1 }` on a cube with one 3 mm
  hole: the plug is 3 − 2 × 0.1 across (± the plug test's tolerance); `edits_of` with drift 0.05
  writes 0.15 − 0.05 (`PLUG_GAP_MM` stated); existing plug tests pass.
Not now: where the plugs stand (P6), MCP's drift (orchestrator).

### P1. Points asked stand before the run, as seeds (item 5)
Window: the link stands each `at_mm` contact in the active group (`stand_points`,
  encrust-app/src/link/plate.rs:429, 445-464) before the run, which takes every contact standing as
  a seed (supports/start.rs:61-64). The edit adds them after the run as `Reason::Steady` needs
  (core-document/src/supports.rs:243-260).
Read: `crates/core-document/src/supports.rs` (+ tests), `crates/encrust-app/src/link/plate.rs`
  (`start_supports`, `stand_points`), `crates/encrust-app/src/supports/start.rs`, `crates/core-supports/src/model/points.rs`
Touch: `crates/core-document/src/supports.rs` (+ tests), `docs/design/document.md`. link/plate.rs is
  552 lines: first move its Supports arm (`start_supports` … `start_inside`, 404-538) into
  `link/plate/supports.rs`, own commit, `cargo xtask hygiene --bless`.
Do: `needs_asked` adds the surface point nearest each `at_mm` to the object's supports in the
  asked group first, then finds needs with them among the seeds; `only_at` stands them alone.
Done when: a 10 mm cube lifted 5 mm, `Supports { at_mm: [(5, 5, 5)] }`: one contact within 2 mm
  of (5, 5, 5) (two before); `only_at` still gives exactly
  one; existing tests pass.
Not now: their shape (the group's profile, as the window's).

### P2. What shows is read only for a clean surface (item 6)
Window: a run reads the painted shown region only when Clean surface is on or strokes are given
  (`task.shown_only_if(clean)`, supports/start.rs:59, job/place.rs:192; link/plate.rs:433). The
  edit always reads it (core-document/src/supports.rs:329).
Read: `crates/core-edit/src/edit.rs` (+ tests), `crates/core-document/src/supports.rs` (+ tests),
  `crates/encrust-app/src/link/plate/supports.rs` (after P1), `crates/encrust-app/src/job/place.rs`
Touch: those, `docs/design/document.md`, `docs/cli.md`
Edit: `Supports` gains `#[serde(default)] clean_surface: bool`; `found` reads `shown_painted` only
  when `clean_surface || !shown_at.is_empty()`. The link's `clean` is the edit's flag or strokes,
  not the Orient tool's toggle (the front end writing the edit sets it from that toggle).
Done when: a 10 mm cube lifted 5 mm with its underside painted shown: `clean_surface: false` gives
  the same contact count as unpainted, `true` fewer (counts stated); a link test; existing tests pass.
Not now: the inside pass's flag (link/plate.rs:535 already takes the toggle alone).

### P3. Which profile a supports edit builds to (items 3, 4)
Window (panel): the group in hand's profile, or the automatic profile in hand sized into its shapes,
  both out of the catalogue with the user's folder (supports/run.rs:77-85, start.rs:42-47); a
  profile chosen becomes the group in hand (`take_up_chosen`, groups.rs:208). The link takes a
  shipped preset only (link/plate.rs:424); the edit a plate group by name, else shipped, without
  making it active (`group_for`, supports.rs:278), and sizes `auto` against `Catalogue::bundled`
  (supports.rs:193).
Read: `crates/core-document/src/supports.rs` (+ tests), `crates/core-engine/src/project.rs` (`SupportState`,
  `AutoSizing`), `crates/encrust-app/src/supports/{groups.rs,run.rs}`, `crates/encrust-app/src/link/plate/supports.rs`,
  `crates/printer-profiles/src/{auto.rs,catalogue.rs}`
Touch: those, `docs/design/document.md`
Do: a name resolves to the plate's group of that name, else a shipped preset, and that group
  becomes the plate's active one; `auto`'s shapes are the plate's groups of their names where
  present, else the shipped ones. The link resolves in the same order. A user-folder profile
  reaches an edit as a group its front end put on the plate first (the window has it; the CLI in R6d).
Done when: a plate with a group "mine" (a profile shipped nowhere, tip 0.3 mm): `Supports { profile:
  "mine" }` stands tips of 0.3 ± 1e-6 and leaves "mine" active; an unknown name → `NoProfile`; a link
  test of the same; existing tests pass.
Not now: merges.

### P4. Orient weighs the supports the panel would build (item 7)
Window (panel, reference): `orient_plans` (supports/groups.rs:84): the automatic profile in hand,
  sized, else the active group's profile, on the model's raft or the plate's, its `z_lift_mm` the
  model's asked lift (`lifted`, job/orient.rs:209); `turn_of` (job/orient.rs:284-292) takes overhang,
  spacing and lift from its `placing()`. The link uses the same; the prepare uses `automatic_plans`
  (prepare/step.rs:118) — the panel wins, R6c follows. The edit takes the active profile alone
  (core-document/src/orient.rs:129).
Read: `crates/core-document/src/{orient.rs,supports.rs}` (+ `orient/tests.rs`), `crates/encrust-app/src/supports/groups.rs`
  (`orient_plans`, `plans_by`), `crates/encrust-app/src/job/orient.rs`, `crates/core-engine/src/project.rs`
Touch: `crates/core-document/src/{orient.rs,supports.rs}` (+ tests), `crates/encrust-app/src/supports/groups.rs`,
  `crates/encrust-app/src/job/orient.rs`
Do: one fn in core-document, the plan an object's orient weighs (the plate's kept sizing sized,
  else the active group's; raft; lift), and one making `OrientSettings` from a plan's `placing()`;
  `plans_by` and `turn_of` call them, `apply_orient` too.
Done when: a plate keeping an automatic sizing: the orient settings take its lightest shape's
  overhang and spacing (equal to the profile's, ± 1e-6); without one, the active group's; a model
  with an asked lift of 3: settings' lift from `z_lift_mm` 3; R6a's cuboid test passes.
Not now: the orient trial (a decision for the user, see the end).

### P5. Orient stands the model where the window does (item 9)
Window: `stand` (encrust-app/src/orient.rs:440-456): turned keeping the middle of its box over the
  same spot, slid onto the plate where it hangs off, dropped onto it, lifted by the placing profile's
  `z_lift_mm` where the pose is not on the plate. The edit keeps the centre of mass and drops
  (`oriented_to`, core-document/src/placement.rs:36-72).
Read: `crates/encrust-app/src/orient.rs`, `crates/encrust-app/src/scene/object.rs` (`onto_plate`:167,
  `slide_onto_plate`, `stand_on_plate`), `crates/core-document/src/{placement.rs,orient.rs}` (+ tests),
  `crates/core-edit/src/edit.rs` (+ tests), `crates/encrust-app/src/link/plate/orient.rs`
Touch: those, `docs/design/document.md`
Edit: `Orient` gains B2's pair, `#[serde(default)] plate_mm: Option<[Scalar; 2]>, low_mm: [Scalar; 2]`
  (no slide where `None`). `onto_plate` and the stand move into `placement.rs` (`stood_turned`); the
  window's `stand` calls it; `apply_orient` uses it with `choice.oriented.on_plate` and P4's lift.
Done when: a 10 × 10 × 30 cuboid standing at box middle (15, 15), `Orient { goal: shortest-print }`:
  box middle stays (15, 15) ± 1e-3; with an area it overhangs, its box lands inside ± 1e-3; a pose
  off the plate stands at the lift ± 1e-3; existing orient tests pass. Rotate keeps the centre of mass.
Not now: Rotate and Scale (the window turns those about the centre of mass too).

### P6. Plugs stand where the window's tool puts them (item 19, layout)
Window: `lay_out` (encrust-app/src/drain/plugs.rs:357-396) moves each plug to the nearest free spot
  (`nearest_free`, `PLUG_SPACING_MM` clear of what models, supports and rafts cover, `covered`:398)
  inside the plate; one that fits nowhere stays where laid. The edit leaves the row of `laid_beside`.
Read: `crates/encrust-app/src/drain/plugs.rs`, `crates/core-plate/src/spot.rs`, `crates/core-document/src/{drain/plugs.rs,arrange.rs}`
  (`standing_meshes`, the footprint with supports), `crates/core-edit/src/edit.rs`
Touch: those (+ tests), `crates/core-document/src/lib.rs`, `docs/design/document.md`
Edit: `Plugs` gains B2's pair (`None` → the row, as now). The footprint and spot steps move into
  core-document beside `apply_plugs` (one copy; the window's `lay_out` calls it, keeping its names
  and `plug_of`). `Report::Plugged` gains `crowded`.
Done when: a cube with two holes on a 100 × 100 area with a second cube to its right: each plug's
  box is ≥ `PLUG_SPACING_MM` (± 1e-3) from every other box and inside the area; no area → the row
  as before; existing plug tests pass.
Not now: MCP (orchestrator).

### P7. A hole drilled at a point is the click's kind (item 15)
Window: a click and the recipe's points drill `HoleKind::AtDepth` through `drain_at`
  (core-volume/src/model.rs:387, drain/points.rs:30). The edit's `kind` defaults to `Drain`
  (`HoleKind::default()`), `edits_of` writes it (core-recipe/src/edits.rs:121). A click never
  misses a face; the edit's `OffAFace` refusal stays (the window's recipe follows it in R6c).
Read: `crates/core-edit/src/edit.rs` (+ tests), `crates/core-recipe/src/edits.rs` (+ tests),
  `crates/core-document/src/drain.rs` (+ tests), `crates/encrust-app/src/link/plate.rs` (the Drain arm)
Touch: those, `docs/design/document.md`, `docs/cli.md`
Edit: `Drain`'s `kind` defaults to `AtDepth` (`#[serde(default = …)]`); `edits_of` writes `AtDepth`.
Done when: `Drain` with no `kind` on a 20 mm cube hollowed to a 2 mm wall, at its top middle: the
  hole is `AtDepth` and stops at `size.depth_mm` ± 1e-4; `edits_of`'s points → `AtDepth`; existing
  tests pass.
Not now: a hole before hollowing (`Walled`, the CLI's case in D1).

### P8. Orient proves its poses on a printer, the standard one where none is named
Decided: the trial runs on every path. A printer and resin named from the catalogue, else the
  standard printer: `elegoo-saturn-4-ultra` (the machine the window's pictures already use,
  encrust-app/src/app/states.rs:14) with the catalogue's first resin for it, defined once in
  printer-profiles (`STANDARD_PRINTER`, `standard_machine(&Catalogue)`). The document stores no
  printer; the edit names it.
Read: `crates/core-engine/src/trial.rs` (`orient_for_print`, `Trial`), `crates/encrust-app/src/job/orient.rs`
  (221-234, the trial it builds), `crates/encrust-cli/src/pipeline/place.rs` (the CLI's trial),
  `crates/core-document/src/orient.rs`, `crates/printer-profiles/src/catalogue.rs`,
  `crates/encrust-mcp/src/server/asked.rs`, `crates/encrust-mcp/src/instructions.md`
Touch: those, `crates/core-edit/src/edit.rs` (+ tests), `crates/printer-profiles/src/lib.rs`,
  `crates/encrust-app/src/link/plate/orient.rs`, `docs/design/document.md`, `docs/cli.md`, ADR 0471
Edit: `Orient` gains `#[serde(default)] printer: Option<String>, resin: Option<String>` (catalogue ids).
apply: builds the `Trial` from them or the standard machine, with P4's support plan, and runs
  `orient_for_print` with it; the window without a printer chosen passes none (the standard one),
  the window's panel too, so the panel and the edit rank alike; the CLI passes `--printer`/resin or
  none. MCP: a pose the search picks becomes `Edit::Orient` with the job's printer in place of the
  Rotate handover; instructions.md tells the LLM to ask which printer, and to use the standard one
  when the user does not say or will not.
Done when: the cuboid test of R6a passes with the trial (printout present on the chosen pose); an
  unknown printer id → `ApplyError::NoPrinter(id)`; the window with no printer and the edit choose the
  same pose for the same body (a link test); MCP test: a search pose is asked as `Orient`.
Not now: a printer stored in the document.

### D1. Design: the CLI on the Document (R6d)
Who: the orchestrator, or one implementer run on Opus. No code.
Read: `docs/design/{document.md,recipes.md}`, `docs/cli.md`, `crates/encrust-cli/src/{stage.rs,pipeline.rs,plate_edits.rs}`,
  `crates/encrust-cli/src/recipe/{plate.rs,piece.rs}`
Touch: new `docs/decisions/0472-the-command-line-runs-on-the-document.md` (draft), `docs/decisions/README.md`,
  new `docs/design/cli-on-document.md`
Write: the page, sized so an implementer can follow it:
  1. The run: stage models into a `Document` (ids as the window gives them) → each shaping flag
     folds to an `Edit` in the window's order (repair, modelling, orient, hollow, drains / channels /
     plugs, supports, arrange) → `apply` each → `into_project` → slice and write as today. A plain
     slice changes no byte.
  2. A table, every flag of `docs/cli.md`: its edit; or the front end's (printer, resin, exposure,
     drift off the plug gap, usable area from the inset, a user-folder support profile written as a
     plate group first); or a steer (check, gate, pause, save).
  3. The window's way for each CLI difference: 2 no lift by the profile, supports grow where it
     stands; 8 orient on shell + cuts + vents (trial: the user's decision); 10 `--orient-pick` from 1
     or 0; 11-13 arrange after supports with their footprint over the usable area, a no-fit an
     info with what was left out; 16-17 drain and channel points in plate mm where the object
     stands, the search with no planned lift; 19 gap − drift and P6's spot; 22 re-hollow as the
     window does, flags overriding a recipe step's keys, gate and pause as now.
  4. The recipe executor: plate steps through `edits_of` + `apply` on the run's Document, per
     model; Check, Save, Gate, Pause stay steps; retries and readings unchanged.
  5. What is deleted (`stage.rs` put-down, `pipeline.rs`, `hollowing*`, `supports.rs`, `stage/room.rs`
     spread, `recipe/{steps,plate,piece,drain}.rs` parts) and what stays (slice, write, measure,
     estimate, batch).
  6. The card split below, corrected, each ≤ ~10 files, each with its closed-form test and the one
     cross-check: the flags and the same edits through `--edits` give the same `.encrust`.
  7. Open points for the user.
  ADR 0472: the decision (the CLI's model pipeline is the Document's; ADR 0466's phase 5), the
  options weighed (keep the pipeline and patch each difference / one executor), consequences.
Done when: the user has read both; R6d1-8 below are rewritten in full from the page.
Not now: code.

### R6d1-8 — sketched; to be written in full after the design card (D1)
- R6d1 Stage into a `Document` and slice from it; no shaping flag yet; a plain slice's bytes equal.
- R6d2 Modelling flags (`--cut`, `--split`, `--part-off`, `--label`, `--combine`, `--relief`,
  `--repair`, mends) as edits.
- R6d3 Orient flags as `Edit::Orient` (P4, P5; pick base and trial per D1).
- R6d4 Hollow, `--drain-at`, `--channel`, `--drain-plugs` as edits: no lift before drains, points in
  plate mm where the object stands, the gap final (flag − drift), plugs at P6's spot.
- R6d5 Supports flags as `Edit::Supports`: no lift by the profile; catalogue and user profiles as
  plate groups first (P3).
- R6d6 Arrange after supports, their footprint, the printer's usable area; no-fit an info.
- R6d7 The recipe executor's plate steps through `edits_of` + `apply`; flags override keys.
- R6d8 Delete the old pipeline; `docs/cli.md`, `docs/features.md`, `docs/architecture.md`.

## Status (2026-11-20)
Done, uncommitted: F1–F3, B1a, B1b, B2, B3, P1–P8 (tests, clippy, hygiene, arch green); D1's page
`docs/design/cli-on-document.md` and ADR 0472 drafted. The user took every recommendation (2026-11-21):
the page's "Decided" section; R6d1 done (unshaped models staged as a `Document`).

## Status (2026-11-22)
Done, uncommitted: R6d2-R6d8. The command line stages every model as a `Document` and carries
each shaping flag out as an edit (`stage/{document,repaired,placed,modelled,finished}.rs`), the
recipe executor's plate steps through `edits_of` + `apply` (`recipe/{piece,steps,plate}.rs`), the
old pipeline deleted (`put_down`, `hollow_and_cut`, `stand_under`, `stage/ground.rs`,
`recipe/drain.rs`). New edits and fields: `Repair` limits, `Orient` keep-clear (window link and
MCP too), `Hollow` keep-solid, `Drain { kind: Support }`, `MergeSupports`. Open: `inspect` still
places a model by `pipeline/place.rs`; MCP does not yet map keep-solid, raft holes and merges to
edits; the orient search shows a spinner, not its bar.

### Decided by the user (2026-11-19)
- The orient trial runs everywhere: with no printer named, on the standard printer (card P8).
- `--orient-pick` keeps counting from 1; the CLI subtracts 1 into the edit's `pick` (D1).

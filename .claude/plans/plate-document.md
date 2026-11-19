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

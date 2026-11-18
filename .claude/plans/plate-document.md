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

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

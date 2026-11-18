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
   `encrust-app/src/job/cut.rs`; closed-form test: unit cube cut at z 0.5 → two volumes
   0.5 ± 1e-6.
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

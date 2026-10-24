# Workflow rules

## Scope

- Do what was asked and nothing else; later work gets a `TODO(step-N):`, never code.
- Bigger than it looked: say so and propose a split; never ship half of it silently.
- Touch only the files it needs; reformatting, renames and cleanups are their own commit.
- Before an architectural, hard-to-reverse choice — a crate boundary, a file format, a
  threading model, f32 or f64, a new dependency — stop and ask: options, trade-offs and a
  recommendation in a few lines. An obvious default: take it, say which, go on.

## Checks

- While working, `cargo test -p <crate>` only. At the end `cargo fmt --all`, then
  `cargo clippy -p <crate> --all-targets -- -D warnings` and `cargo test -p <crate>` for each
  crate touched.
- Workspace-wide forms are CI's, run here only on request — except after a change to
  `Cargo.lock` or a manifest's dependencies: `cargo hakari generate`, `cargo hakari
  manage-deps`, clippy and test once over the workspace prefixed `ENCRUST_FULL_CHECK=1`, then
  `cargo sweep --time 7` (ADR 0272).
- One cargo at a time. Cut output with `tail`, never a `grep` hiding `Blocking`/`Compiling`.
- Never `rustup update` or move the `rust-toolchain.toml` pin, for a lint or for speed.

## Done

Update `docs/architecture.md` if a crate changed, give each architectural decision its ADR,
review the diff by `review.md` and fix what it finds, propose a commit message. No commit or
push unless asked.

## ADRs

- A change to a crate boundary, data format, dependency or algorithm family without its ADR
  is incomplete. A decision inside one crate gets rustdoc or a `docs/design/` page; decisions
  taken together are one record.
- An uncommitted ADR is a draft: rewrite, renumber or delete it, whatever its Status; never
  supersede it with a second one.
- A committed ADR is frozen but for its Status: a new decision is a new ADR superseding it,
  unless it continues it before anything builds on it — then fold it in.

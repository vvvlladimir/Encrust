# Workflow rules

## One piece of work at a time

- Work what was asked and nothing else. A feature that belongs to later work gets a
  `TODO(step-N):` comment, never code.
- Do not widen the scope. If it is bigger than it looked, say so and propose a split; never
  ship half of it silently.
- Touch only the files it needs. Reformatting, renaming and cleanups are their own commit.

## Ask at a fork

Stop and ask when a choice is architectural and hard to reverse: a crate boundary, a file
format, a threading model, f32 versus f64, a new dependency. Give the options, their trade-offs
and a recommendation, in a few lines. Anything with an obvious default: pick it, say which,
keep going.

## Before it is done

While working, run only `cargo test -p <crate>`. At the end run `cargo fmt --all`, then
`cargo clippy -p <crate> --all-targets -- -D warnings` and `cargo test -p <crate>` for each
crate you touched. The workspace-wide forms in `AGENTS.md` are CI's gate and run here only
when the user asks — with one exception: a change to `Cargo.lock` or to a manifest's
dependencies runs `cargo hakari generate`, `cargo hakari manage-deps`, then the clippy and
test forms once over the whole workspace prefixed `ENCRUST_FULL_CHECK=1`, never crate by
crate, and ends with `cargo sweep --time 7` (0272).

One cargo at a time in this checkout: a second one waits on the first's lock. Cut its output
with `tail`, never a `grep` that hides `Blocking` and `Compiling`. All of it
runs on the compiler `rust-toolchain.toml` pins: never `rustup update` or move that pin to
clear a lint, and never because a check is slow. Then update
`docs/architecture.md` if a crate changed, make sure each architectural decision has its ADR,
read your own diff by `.claude/rules/review.md` and fix what it finds, and propose a commit
message. Do not commit or push unless asked.

## ADRs ship with their code

A commit that changes a crate boundary, a data format, a dependency or an algorithm family
without an ADR is incomplete. A decision that does not leave its crate — no public API, no
data format, no dependency changed — gets rustdoc or a `docs/design/` page instead, and
decisions taken together are one record, not one each.

While an ADR is still in the working diff it is draft material: rewrite, renumber or delete it
freely, even if its Status already reads `Accepted`. Never add a second ADR superseding one
that is not committed yet. Once committed it is frozen: change only its Status, and record a
new decision in a new ADR that supersedes it — except one that continues it, wholly or in
part, before anything else builds on it: fold that into the old ADR instead of adding one.

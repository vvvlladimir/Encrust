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

Run `cargo fmt --all`, then clippy with `-D warnings` and the tests of each crate you
touched, by `-p`. The workspace-wide forms in `AGENTS.md` are CI's gate, not yours: they
rebuild the whole graph, and they are run here only when the user asks for them. All of it
runs on the compiler `rust-toolchain.toml` pins: never `rustup update` or move that pin to
clear a lint, and never because a check is slow. Then update
`docs/architecture.md` if a crate changed, make sure each architectural decision has its ADR,
and propose a commit message. Do not commit or push unless asked.

## ADRs ship with their code

A commit that changes a crate boundary, a data format, a dependency or an algorithm family
without an ADR is incomplete. A decision that does not leave its crate — no public API, no
data format, no dependency changed — gets rustdoc or a `docs/design/` page instead, and
decisions taken together are one record, not one each.

While an ADR is still in the working diff it is draft material: rewrite, renumber or delete it
freely, even if its Status already reads `Accepted`. Never add a second ADR superseding one
that is not committed yet. Once committed it is frozen: change only its Status, and record a
new decision in a new ADR that supersedes it.

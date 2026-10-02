# 0173. A pull request runs Linux, and main runs the rest

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

Every pull request ran fourteen jobs: the checks on three platforms, a rust-version floor and a
rustdoc build, each compiling wgpu and egui from its own cache, plus two minutes of fuzzing per
target whenever `fuzz/` changed. A Monday of Dependabot pull requests queued forty jobs on runners
that allow five macOS legs at once, and main repeated all of it after the merge.

## Decision

A pull request runs one Linux job — fmt, clippy, tests, rustdoc with `-D warnings` — and the secret
scan. macOS, Windows and the rust-version floor run on a push to main, and on any branch through
`workflow_dispatch` when a change is worth checking there before the merge. Linux also runs on
main, because a pull request restores its cache only from there. A change only to `docs/**` or
`*.md` runs none of it.

A pull request touching `fuzz/` builds every target in one job. Fuzzing itself runs weekly and on
demand. This replaces the trigger in ADR-0161; the rest of that record stands.

## Consequences

A pull request costs two jobs instead of fourteen and answers in one Linux build. The cost is that
a macOS- or Windows-only break, or a use of a newer standard-library item, is found on main rather
than before the merge. Reopen this if that happens often enough that main is red more than it is
green; the merge queue below is the next step.

## Alternatives considered

### A merge queue running the full matrix on `merge_group`

Keeps main always green on every platform, but adds a queue to every merge for a project with one
maintainer.

### Linux on a pull request, the rest on main

A break on another platform lands on main and is fixed by a follow-up commit, which is the price
of not waiting on a macOS runner for every pull request.

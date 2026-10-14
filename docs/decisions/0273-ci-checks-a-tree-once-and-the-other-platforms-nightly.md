# 0273. CI checks a tree once, and the other platforms nightly

- **Status:** Accepted
- **Date:** 2026-10-14

## Context

Under ADR-0173 a squash merge of a pull request that had just passed reran Linux on main over the
same tree, and every push to main added macOS, Windows, the threaded web build and the
rust-version floor: about 25 runner-minutes per merge, Windows alone eight. Each job wrote its own
cache for every new `Cargo.lock`, so the repository sat at 11 GB against GitHub's 10 GB and its
caches evicted one another. A release restored nothing — its caches from the release before were
long gone — and each leg spent up to two minutes compiling `cargo-packager`, as CI did for
`cargo-hakari`, `cargo-about`, `cargo-fuzz` and `wasm-bindgen-cli`.

## Decision

- Linux runs on a pull request and on a push to main, but a push is skipped when an earlier run's
  Linux job passed on the same git tree. A run passing is not enough: a draft's run passes with
  every job skipped.
- macOS, Windows, the threaded web build and the floor run nightly, only when main moved since the
  last nightly run that passed, and on any branch through `workflow_dispatch`. The nightly run
  also runs Linux, which keeps main's cache fresh when merges were skipped.
- A change only to `docs/`, Markdown, `.claude/` or the issue templates runs none of it, the
  nightly run included.
- Only main saves a cache. The licence check in `audit.yml` reads Linux CI's cache rather than
  keeping its own; the release workflow keeps none.
- A tool is downloaded prebuilt by `taiki-e/install-action`, never compiled by `cargo install`.
- Fuzzing keeps ADR-0173's trigger: weekly and on demand, and a pull request touching `fuzz/`
  builds every target in one job.

## Consequences

A merge of an up-to-date pull request costs a gate job instead of six jobs. A macOS-, Windows- or
browser-only break is found by the next morning rather than right after the merge, and a release
cut the same day as such a break still builds from it: dispatch CI on the release pull request
first when the day's merges touched platform code. Reopen this if a nightly failure lands in a
release.

A pull request's second push restores main's cache rather than its own first push's, so it
compiles somewhat more. Tool versions are whatever is latest, as with `cargo install` before.

## Alternatives considered

### Keep ADR-0173 and only raise the cache limit

Storage above 10 GB is billed, and it leaves the duplicate runs and the compiled tools in place,
which cost more than the evictions.

### `fkirc/skip-duplicate-actions`

Dedupes by tree as well, but counts a run as passed by its conclusion, so a draft's empty run
would let the ready pull request skip Linux.

### Run the other platforms nightly, and dedupe by tree

The cost is the day's delay above. The tree is compared with a pull request's head, not the merge
commit its run tested, so a branch behind main is never skipped, and one that is up to date is
skipped only when its tree is exactly what main became.

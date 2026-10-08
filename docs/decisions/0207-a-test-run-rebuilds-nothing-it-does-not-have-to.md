# 0207. A test run rebuilds nothing it does not have to

- **Status:** Accepted
- **Date:** 2026-10-08

## Context

`cargo test --workspace` is the gate on every piece of work and is run after most edits.
It had reached 9–10 minutes, and a run that changed nothing cost the same as a run that
changed everything. Three causes were measured, none of them the tests: all 69 test
binaries together execute in 22.1 s, the slowest being the window's offscreen render tests
at 6.6 s and the `net-sdcp` socket tests at 8.0 s.

The first is `printer-profiles/build.rs`. It printed `cargo:rerun-if-changed` for
`assets/profiles/resins`, a directory that does not exist, because a resin is measured and
not shipped (0196). Cargo reruns a build script whose watched path is missing, every time,
so `printer-profiles` and everything downstream of it — up to `encrust-app` and its window
and GPU stack — recompiled and relinked on every invocation. A scratch crate confirms the
rule: watching a missing path recompiles on each `cargo build`, watching an existing one
never does.

The second is rustdoc. `cargo test` compiles a doctest target per library crate and does
not fingerprint that work, so all 28 of ours were compiled on every run — 43 s for zero
doctests, because rustdoc here is one or two lines of prose and carries no examples.

The third is debug information. `[profile.test]` inherited the full set, which is most of
what a test link writes to disk.

## Decision

A build script watches only paths that exist. Every library crate sets `[lib] doctest =
false`. `[profile.test]` sets `debug = "line-tables-only"`.

## Consequences

A run that changes nothing is 23 s, of which 22 s is the tests themselves; it was 9–10
minutes. What remains is proportional to the edit, which is what it should be.

A rustdoc example would now be neither compiled nor run. `.claude/rules/code-style.md`
asks for prose rustdoc, so nothing is lost today; the first crate that wants an executable
example in its rustdoc is the signal to reopen this, and it reopens for that crate alone.

A backtrace from a failing test still names file and line. A debugger attached to a test
binary has less to work with, which is a trade taken knowingly: tests are debugged by
running them, not under a debugger.

## Alternatives considered

### Delete or split the slow tests

The premise was that some test was slow. Measuring each binary showed the opposite — the
whole suite is 22.1 s — so there was nothing to delete, and deleting coverage would have
hidden the build problem rather than fixed it.

### A faster runner, or a narrower command in `AGENTS.md`

`cargo test --workspace --lib --bins --tests` skips the doctest phase, and a parallel
runner would overlap the 22 s of execution. Both leave the cost in place for anyone who
types the plain command, which is what gets typed. The fix belongs in the manifests.

### The option that won, and what it costs

`doctest = false` is 28 lines in 28 manifests saying the same thing, and a new crate that
forgets it silently pays the cost again. Cargo has no workspace-wide setting for it, so
the duplication is the price of making the plain command fast.

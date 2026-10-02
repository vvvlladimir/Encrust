<!-- Keep whichever lines apply and delete the rest. A one-line fix does not need a long form. -->

## What this changes

<!-- What the user can now do, or what stopped being wrong. Link the issue with "Closes #NN". -->

## Why this way

<!-- Only if the approach is not the obvious one, or if you considered something else and dropped
     it. Skip this whole section otherwise. -->

## Checks

- [ ] `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and
      `cargo test --workspace` pass.
- [ ] I read the rule file in [`.claude/rules/`](../.claude/rules/) for the area I touched.
- [ ] Nothing in a `core-*` crate reached for `egui`, `wgpu` or `winit`, and no new edge appeared
      that is not drawn in [`docs/architecture.md`](../docs/architecture.md).

## Obligations this change may carry

- [ ] **Tests.** New geometry is tested against a body with a closed-form answer, with an explicit
      tolerance and where the expected number comes from. A fixed defect has a test that fails
      without the fix.
- [ ] **Benchmark.** This touches a hot path, so the pull request carries before-and-after numbers
      from a `criterion` benchmark in `benches/` — or it touches none.
- [ ] **ADR.** This moves a crate boundary, fixes a data format or unit, adds a dependency or swaps
      an algorithm family, so it adds one in [`docs/decisions/`](../docs/decisions/) in this same
      change — or it does none of those, and needs none.
- [ ] **Documentation.** Format detail went to [`docs/formats/`](../docs/formats/), an algorithm to
      [`docs/design/`](../docs/design/), a CLI flag to [`docs/cli.md`](../docs/cli.md) — and the
      code kept at most a one-line pointer, not the explanation.
- [ ] **A printer.** This adds or changes a profile in `assets/profiles/`, and says plainly whether
      a real machine printed the result or the numbers came from a specification.

## Anything a reviewer should know

<!-- A screenshot for a visual change. A number you are unsure about. A follow-up you deliberately
     left out. "Nothing" is a fine answer. -->

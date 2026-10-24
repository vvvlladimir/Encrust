# Review rules

A review — `/code-review`, a self-check before saying a step is done, a reviewer agent — holds
the diff to every rule in `.claude/rules/` and `CLAUDE.md`. A broken rule is a finding, cited
as `file:line` and the rule it breaks; style already enforced by clippy or `rustfmt` is not.

Look for these first; each has slipped through before and cost a refactor to undo:

1. **Structure.** A `core-*` crate reaching a UI crate, an edge `docs/architecture.md` does
   not draw, a type copied instead of moved down, logic or test scaffolding in `lib.rs`.
2. **Size.** A function the diff takes past 40 lines, a file past 400 lines of non-test code,
   an inline test module past 150, or any line added to something already over a limit.
3. **Repetition.** A computation the workspace already has — a grid cell, a matrix inverse
   with its singular check, a test body — written again instead of called.
4. **Scale.** A loop over every face, voxel or support where a `Bvh`, `Buckets` or `FastMap`
   answers; a pass over pairs; a fixed-step walk or retry with no bound.
5. **Comments.** More than two lines, a restatement of the code, a doc saying how instead of
   what, a bare `TODO`.
6. **Scope.** Code for a later step, a cleanup mixed into a feature, a user-visible change
   without `docs/features.md`, a decision without its ADR.
7. **Tests.** A fix without a test that fails before it, an error variant nothing triggers,
   geometry checked without a closed-form answer and tolerance.

A performance finding needs a reason the cost grows — with what, and by how much — not a
feeling that it is slow.

# Review rules

Every review — `/code-review`, a self-check before "done", a reviewer agent — holds the diff to
`.claude/rules/` and `CLAUDE.md`. A finding cites `file:line` and the rule broken; what clippy
or `rustfmt` enforce is not reported. Check these first:

1. **Structure:** a core reaching a UI crate, an edge `docs/architecture.md` does not draw, a
   type copied instead of moved down, logic or test scaffolding in `lib.rs`.
2. **Size:** a function past ~40 lines that could split at a named step, a file past ~400
   holding two concerns, anything added to what is already over; the hard limits are xtask's.
3. **Repetition:** a grid cell, a matrix inverse, a test body or any computation the
   workspace has, written again.
4. **Scale:** a loop over every face, voxel or support, a pass over pairs, a fixed-step walk or
   an unbounded retry.
5. **Comments:** over two lines, restating code, how instead of what, a bare `TODO`.
6. **Scope:** later work, a cleanup inside a feature, a visible change without
   `docs/features.md`, a decision without its ADR.
7. **Parity:** a window feature `encrust-cli` cannot reach.
8. **Window:** a break of `docs/design/ui-standards.md`.
9. **Tests:** a fix without a failing test first, an untriggered error variant, geometry
   without a closed form and tolerance.

A performance finding states what the cost grows with and by how much.

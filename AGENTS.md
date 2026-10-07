# Working on Encrust

Offline MSLA/SLA slicer for resin printers, in Rust. Mesh in, layers out, exposure masks
rasterised, printer file written. Pre-alpha; the pipeline runs end to end — import, orient,
hollow, support, slice, rasterise, analyse, write, send.

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo bench --workspace
cargo run --release -p encrust-cli --bin encrust -- slice model.stl \
  --printer elegoo-mars-4-ultra --resin my-grey --center -o model.goo
cargo xtask gen-profiles --source <dir of .ini profiles> --dry-run
cargo xtask web   # the window for a browser, on nightly: docs/design/web-build.md
cargo xtask man   # the command line's man pages, into target/man
```

The first three pass before any piece of work is done.

**The rules bind every edit and live in [`.claude/rules/`](.claude/rules/):**
`architecture.md`, `code-style.md`, `testing.md`, `documentation.md`, `workflow.md`. Read
them before changing anything, and [`CLAUDE.md`](CLAUDE.md) for the one rule above them. An
ADR citing "rule N of `AGENTS.md`" means the numbered rule in
`.claude/rules/architecture.md`.

`docs/architecture.md` says which crate owns what and which dependency each one may have —
read it before moving a type or adding a crate. Which kind of material belongs in which file:
`.claude/rules/documentation.md`. The Elegoo `.goo` specification and open-source readers of
the other containers are the sources for file format detail.

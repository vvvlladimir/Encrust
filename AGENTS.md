# Working on Encrust

Offline MSLA/SLA resin slicer in Rust: mesh in, layers out, masks rasterised, printer file
written. Pre-alpha; import, orient, hollow, support, slice, rasterise, analyse, write and send
run end to end.

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
cargo xtask arch  # the drawn dependency graph against the manifests
cargo xtask hygiene  # size limits by ratchet; --bless after a split shrinks the baseline
cargo xtask app   # the window as Encrust Dev.app, which macOS lets onto the local network
cargo xtask i18n ru  # what a translation of the window lacks: docs/design/i18n.md
cargo xtask licenses  # THIRD-PARTY-LICENSES.md from the lockfile (needs cargo-about)
```

CI runs the first three over the workspace; locally the per-crate forms in
`.claude/rules/workflow.md` apply.

**Every edit follows [`.claude/rules/`](.claude/rules/)** — architecture, code-style,
testing, documentation, workflow, review, and ui for the window — and [`CLAUDE.md`](CLAUDE.md)
above them. "Rule N of `AGENTS.md`" in an ADR is rule N of `architecture.md`. Which crate owns
what and may depend on what: `docs/architecture.md`. Format sources: the Elegoo `.goo`
specification and open-source readers of the other containers.

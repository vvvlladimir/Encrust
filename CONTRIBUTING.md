# Contributing to Encrust

Thanks for looking. Encrust writes files that machines cure resin from, so the bar for a change is
"I can show that this is right", not "it compiles".

Before a large change, open an issue or a discussion first. A refused pull request wastes more of
your evening than a refused idea.

Taking part here means following the [Code of Conduct](CODE_OF_CONDUCT.md).

## The most useful thing you can do

**Tell us about your printer.** Encrust ships five printer profiles, and exactly one of them has
ever printed anything. Everything else about this project can be fixed by writing code; this cannot.

Open a **printer profile** issue with:

- the printer's make and model, and its screen resolution and build volume if you know them;
- a small file its own slicer produced — `.ctb`, `.goo`, `.pwmx`, `.sl1`, whatever it takes. A
  10 × 10 × 10 mm cube is plenty, and the file tells us the header fields, the layer encoding and
  the defaults that machine expects;
- the resin and the exposure settings you actually print with.

If you have already tried Encrust on it, say what happened — including "it printed fine", which is
just as useful as a failure and far rarer in a bug tracker.

## Getting set up

```sh
git clone git@github.com:vvvlladimir/Encrust.git
cd Encrust

cargo run --release -p encrust-app --bin encrust-gui  # the window
cargo run --release -p encrust-cli --bin encrust -- --help
```

`rust-toolchain.toml` pins the toolchain, so `rustup` installs the right one on the first build.
You need stable Rust 1.96 or newer and, for the window, a GPU with Vulkan, Metal or DirectX 12.
On Linux the development headers `eframe` links against are needed to build — on Debian or Ubuntu
`libgtk-3-dev`, `libxcb-render0-dev`, `libxcb-shape0-dev`, `libxcb-xfixes0-dev`, `libxkbcommon-dev`
and `libssl-dev`, which is the list [`.github/actions/linux-gui-deps`](.github/actions/linux-gui-deps/action.yml)
installs in CI.

A release build matters here: a debug build of the slicing path is slow enough to look hung.

## Before you open a pull request

Everything below must pass. It is what CI runs, and it runs offline.

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

If you touched a reader — a mesh loader, a sliced-file container, a run-length decoder — run its
fuzz target for a minute as well; [`fuzz/README.md`](fuzz/README.md) says how, and a crash is a
finding worth more than the feature you were adding.

If you touched a hot path, `cargo bench --workspace` as well, and put the before-and-after numbers
in the pull request. A performance claim without them is not accepted — that rule is in
[`.claude/rules/testing.md`](.claude/rules/testing.md) and it applies to everybody.

## How this repository is organised

Read [`AGENTS.md`](AGENTS.md) first: the commands, and where everything else is. Which crate owns
which types and where a given kind of code belongs is in
[`docs/architecture.md`](docs/architecture.md); the rules each area is reviewed against are in
[`.claude/rules/`](.claude/rules/) — architecture, code style, testing, documentation, workflow.
Read the file for the area you are touching before you touch it; most comments on a first pull
request are things written down there.

The rules with the most teeth:

- **The dependency graph in [`docs/architecture.md`](docs/architecture.md) is exact.** Anything not drawn there is forbidden, including a sideways dependency between two
  peers. No `core-*` crate may touch `egui`, `wgpu` or `winit`; `grep -rn egui crates/core-*` must
  stay empty.
- **Cost follows the work, not the model.** Geometry queries go through the `Bvh`, neighbourhood
  searches through a grid, anything proportional to triangles or layers runs on `rayon`, and peak
  memory must not grow with the layer count.
- **No `unwrap()` or `expect()` in library code** outside `#[cfg(test)]`. Libraries return a
  `thiserror` enum whose variants carry what the caller needs to act; binaries use `anyhow`.
- **Geometry is tested against a body with a closed-form answer** — a unit cube, a sphere of
  radius `r`, a regular tetrahedron — with an explicit tolerance and an assertion message saying
  where the expected number comes from.
- **A bug fix comes with a test that fails before it.**
- **Comments are for what the code cannot carry**: a tolerance and why that value, a clause of a
  file format, a required ordering. Not a restatement of the line below.
- **A decision that moves a crate boundary, changes a stored format, adds a dependency or swaps an
  algorithm family needs an ADR** in [`docs/decisions/`](docs/decisions/), in the same commit as
  the code. A routine fix does not.

## File formats and the law

Encrust reads and writes other people's formats so that a printer somebody already owns will accept
a file. That is interoperability, and it is what the project is for.

Two things follow, and they are not negotiable here:

- **Only publicly documented or independently reverse-engineered detail.** Do not paste code,
  headers or constants out of a vendor SDK whose licence forbids it, and do not submit anything you
  obtained under an NDA.
- **Do not bulk-import another slicer's profile catalogue.** A profile built from a specification,
  from a file your own printer accepted, or from your own measurements is fine. A wholesale copy of
  somebody else's database is not.

Format detail that is worth keeping goes in [`docs/formats/`](docs/formats/), with the code holding
at most a one-line pointer to it.

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org/):

```
fix(format-chitu): write the layer table before the preview

The v5 header stores an absolute offset to each layer, and a thumbnail
written first shifts every one of them.
```

If the change carries an ADR, end the body with `Refs: ADR-NNNN`.

Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`, `chore`. The scope is the
crate or the area: `geometry`, `slicer`, `raster`, `supports`, `volume`, `plate`, `analysis`,
`format`, `format-goo`, `format-chitu`, `net-sdcp`, `profiles`, `app`, `cli`, `docs`. The subject is
imperative, at most 72 characters, no full stop.

Say *why* in the body. What changed is visible in the diff; why it had to is not.

## Releases

Nobody tags by hand. A bot keeps one open pull request holding the next version number and the
changelog entries earned since the last release; merging it writes `CHANGELOG.md`, bumps the version,
tags, and opens a draft release. The archives are built from that tag, attached to the same draft,
signed, and named in `latest.json`, the feed the window's update check reads. A maintainer publishes
the draft after looking at it, and only then does the window offer it. Signing needs the
`UPDATE_SIGNING_KEY` repository secret; see [`docs/design/updates.md`](docs/design/updates.md#the-signing-key).

That is why the commit subjects matter: the changelog is made of them. See
[ADR-0162](docs/decisions/0162-release-please-cuts-the-release.md).

## AI-assisted contributions

They are welcome, and this repository was itself built with heavy use of them. The condition is
simple: you have read every line you are submitting and you can defend it in review. A pull request
whose author cannot explain why a transformation is correct will be closed regardless of who or what
wrote it — the same standard as for hand-written code, applied honestly.

## The contributor licence agreement

By contributing you agree to the [Contributor License Agreement](.github/CLA.md); a bot will ask you
to sign it once, on your first pull request. It does not take your copyright away. It lets Encrust
be offered under a paid licence alongside the AGPL without asking every contributor again — the
reasoning is in [ADR-0159](docs/decisions/0159-agpl-with-a-contributor-licence-agreement.md).

## Security

Do not open a public issue for a vulnerability. See [SECURITY.md](SECURITY.md).

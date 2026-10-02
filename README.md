<div align="center">

<img src="assets/icon/encrust.svg" alt="" width="96" height="96">

# Encrust

### Slice resin prints on your own machine, with nothing in between.

Encrust is a free, open-source slicer for MSLA and SLA resin 3D printers: a mesh goes in, layers
come out, every layer is rasterised into an exposure mask, and a file your printer already
understands is written. It runs entirely on your computer, needs no account, and sends nothing
anywhere — except the finished file to your printer, when you ask it to.

Windows 10/11 64-bit, macOS 12+, Linux x86-64. Licensed under AGPL-3.0.

[Build from source](CONTRIBUTING.md#getting-set-up)
&nbsp;·&nbsp; [Architecture](docs/architecture.md)
&nbsp;·&nbsp; [Discussions](https://github.com/vvvlladimir/Encrust/discussions)

[![CI](https://github.com/vvvlladimir/Encrust/actions/workflows/ci.yml/badge.svg)](https://github.com/vvvlladimir/Encrust/actions/workflows/ci.yml)
[![License: AGPL v3](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](#get-started)
[![Status: pre-alpha](https://img.shields.io/badge/status-pre--alpha-red.svg)](#status)

</div>

## Is it for you?

Encrust is for people who print resin and have got tired of what preparing a print now costs them:
an account to log into, a subscription in front of automatic supports, a window that takes ten
seconds to open a model, and no way to find out what the slicer actually decided.

It is not trying to be the biggest slicer. It is trying to be one you can read, script and check:
the same code behind a window and behind a command line, a report you can diff, and an answer to
*why* a layer is risky rather than a red dot on a slider.

**Today it is pre-alpha** — see [Status](#status) before you point it at a print.

## Why Encrust

**Offline, and provably so.** No account, no telemetry, no crash reports. Encrust talks to a printer
on your own network when you press the button, and to GitHub only if you turn on the update check.
The code is here, so that is a fact you can check rather than a promise you have to take.

**It tells you what will happen.** Encrust does not stop at "island detected". It simulates the
stack it just wrote — what each pixel's accumulated exposure actually cures, where a layer is held
by too little, where suction will pull a part off the plate — and reports that per layer, with the
measurement behind it.

**One engine, two front ends.** The window and the `slice` command run the same pipeline. Anything
you can do by clicking, you can do over a directory of models in a script, and get a JSON report
per model. No slicer in this class has a usable command line.

**It reads files, not just writes them.** Open a `.ctb`, `.goo`, `.pwmx`, `.sl1`, `.zip`, `.cxdlp`,
`.svgx` or `.cws` someone else sliced — including ones Encrust cannot write yet — and look at its
layers, its settings and its thumbnails.

**Fast because of how it is built, not because of a progress bar.** Rust, `rayon` across layers and
triangles, a BVH for every geometric query, sparse data kept sparse, and layers streamed so peak
memory does not grow with the layer count. Where a claim about speed appears, there is a
`criterion` benchmark behind it.

## What's inside

**Slicing and exposure**
- Contour slicing with closed-contour guarantees, adaptive layer height by curvature, and several
  cutting planes to one layer
- Exposure by height, anti-aliasing with grey levels and blur, per-layer masks as run-length data
- Textures pressed into the mesh as relief, with UVs read from OBJ and 3MF

**Supports**
- Manual placement, automatic placement, branching trunks, rafts and bracing
- Supports that keep out of the model and hold what they were put under
- Painted patches, per-part editing, and contact tuning

**Hollowing and geometry**
- Signed-distance field hollowing with infill, drain holes and trapped-resin detection
- Mesh diagnosis and repair: welding, holes, non-manifold edges, degenerate faces, winding
- Cut and split, auto-orientation, auto-arrange, multi-plate layouts, batch preparation

**Analysis**
- What the written stack cures, layer by layer, and where it is likely to fail
- Island, suction and cross-section reporting with the numbers that produced it

**Files and printers**
- Reads STL, OBJ and 3MF, with UVs and one texture per material
- Writes `.goo`, `.ctb` v4/v5, `.cbddlp`, `.photon`, `.pwmx` and sixteen more Anycubic
  extensions, `.sl1`, `.sl1s`, the `.zip` of greyscale PNGs, `.cxdlp` v3/v4, `.svgx` and `.cws`
- Reads every one of those back
- Sends over your local network: SDCP versions 3 and 1 (Elegoo and Chitu boards), PrusaLink over
  HTTP
- `.encrust` project files, printer, resin and support profiles as plain TOML you can edit

## Printers

Five printer profiles and four resin profiles ship today:

| Printer | Format | Verified by printing |
|---|---|---|
| ELEGOO Saturn 4 Ultra | `.goo` | **Yes** |
| ELEGOO Mars 4 Ultra | `.goo` | No |
| ELEGOO Mars 3 Pro | `.ctb` | No |
| Phrozen Sonic Mini 8K | `.ctb` | No |
| Uniformation GKtwo | `.ctb` | No |

"No" means the profile was written from the format specification and checked by reading the written
file back, not by curing resin. **Treat those four as unverified.** Encrypted `.ctb`, which some
newer Chitu boards require, is not written yet.

If your printer is missing, that is the most useful thing you can help with — see
[Contributing](#contributing).

## Get started

There are no binary releases yet; the first tagged release will appear on the
[releases page](https://github.com/vvvlladimir/Encrust/releases). Until then, build it:

```sh
git clone git@github.com:vvvlladimir/Encrust.git
cd Encrust
cargo run --release -p encrust-app --bin encrust
```

Or from the command line, without a window:

```sh
cargo run --release -p encrust-cli --bin slice -- model.stl \
  --printer elegoo-saturn-4-ultra --resin standard-grey --center -o model.goo
```

You need a recent stable Rust toolchain (edition 2024, 1.96 or newer — `rust-toolchain.toml` pins
it) and a GPU that supports Vulkan, Metal or DirectX 12 for the window. Every flag the CLI takes is
in [`docs/cli.md`](docs/cli.md).

## Status

**Pre-alpha, version 0.0.1.** The pipeline is complete end to end — import, orient, hollow,
support, slice, rasterise, analyse, write, send. It is covered by unit, property and
integration tests — around 1500 of them — and CI runs the whole suite on Windows, macOS and Linux
for every commit to `main`.

What that does **not** mean:

- Only one printer, an ELEGOO Saturn 4 Ultra, has ever printed a file Encrust wrote. The other four
  profiles are unverified, and so is every printer not in the list.
- Exposure and lift settings in the shipped resin profiles are starting points, not calibrated
  values. Run a calibration print before trusting them.
- Nothing here has been through the kind of use that finds the last bugs. Keep a slicer you trust
  installed, and compare the two before a long print.

A failed resin print costs a few hours and a few millilitres. Treat early Encrust as capable of
costing you that.

## Privacy

Encrust makes exactly these network connections:

| What | Where it goes | When |
|---|---|---|
| A sliced file | The printer's own IP on your network, over SDCP or PrusaLink | Only when you press send |
| Printer discovery | A UDP broadcast on your local network | Only when you ask it to look for printers |
| Update check | `github.com`, for this project's newest release, over HTTPS | Once a day if you turn it on in Settings › Updates, or when you press "Check now"; off by default |
| Update download | `github.com`, the release archive for your platform | Only when you press "Install and restart" |

The update check sends nothing but the request itself: GitHub sees your address and the version you
run. A download is installed only if it carries the project's own signature. There is nothing else —
no analytics, no crash reporting, no licence check, no account, no cloud. Models, profiles and
projects are files on your disk.

## FAQ

**Is Encrust free?**
Yes, and everything that runs on your machine will stay free and open source under the AGPL-3.0. If
anything paid ever appears it will be something that genuinely costs money to provide — not a
paywall in front of what already works.

**Will it work with my printer?**
If your printer takes one of the formats listed above, probably — but see the verified column, and
test with a small print. If it needs encrypted `.ctb`, not yet.

**Does it replace the slicer you use today?**
No. The established ones have hundreds of verified printer profiles and years of real prints behind
them; Encrust has one verified printer. What it has instead is an offline guarantee, a command
line, and an analysis of the file it wrote.

**Can it open a file another slicer made?**
Yes, for every container it supports, including ones it cannot write.

**Is it a fork of anything?**
No. It is written from scratch in Rust. The ELEGOO `.goo` specification and open-source
readers of the other containers are the references for format detail.

## Contributing

The single most useful contribution is a **printer profile**: the printer you own, a file its own
slicer produced, and whether what Encrust wrote actually printed. That is the one thing the project
cannot buy its way out of. Open an issue, or see [CONTRIBUTING.md](CONTRIBUTING.md).

Bug reports, failing meshes and pull requests are all welcome. [`AGENTS.md`](AGENTS.md) is the way
in and points at the rules each area is reviewed against, and
[`docs/decisions/`](docs/decisions/) holds the reasoning behind every structural choice.

## Architecture

A Cargo workspace of small crates with a one-way dependency graph: no `core-*` crate may touch a
graphics or windowing library, and only the two binaries may. `core-geometry` knows nothing about
slicing, `core-slicer` nothing about rasterisation, and `core-pipeline` is the one place allowed to
span them on the way to a written file.

Which crate owns what, the graph itself and what it forbids:
[`docs/architecture.md`](docs/architecture.md). The rules that bind it:
[`.claude/rules/architecture.md`](.claude/rules/architecture.md).

## Licence

[AGPL-3.0-only](LICENSE). You may run, study, change and share it; a modified version you
distribute, or offer to others over a network, must carry its source too.

Contributions are covered by a [contributor licence agreement](.github/CLA.md), which keeps the
right to offer Encrust under a second, commercial licence as well. The public source stays open
source — see
[ADR-0159](docs/decisions/0159-agpl-with-a-contributor-licence-agreement.md) for why.

## Disclaimer

Encrust prepares files for machines that cure resin with ultraviolet light. It cannot know whether
the settings you give it suit your resin, your printer or your model, and a wrong exposure or lift
setting damages prints and occasionally hardware. Check what it produces before a long job, and
read your printer's and your resin's own instructions. Provided without warranty of any kind, as
the licence says.

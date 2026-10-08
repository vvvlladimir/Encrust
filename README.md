<div align="center">

<img src="assets/icon/encrust.svg" alt="" width="96" height="96">

# Encrust

### The resin slicer that answers to you.

Free and open source, for MSLA and SLA printers. Supports, hollowing and a layer-by-layer check
that catches a failed print before it costs you resin — all of it on your own computer, with no
account and no cloud.

[**Download**](https://encrust.app/download/)
&nbsp;·&nbsp; [**Try it in a browser**](https://encrust.app/app/)
&nbsp;·&nbsp; [Website](https://encrust.app)
&nbsp;·&nbsp; [Guides](https://encrust.app/guides/)
&nbsp;·&nbsp; [Printers](https://encrust.app/printers/)
&nbsp;·&nbsp; [What's new](CHANGELOG.md)
&nbsp;·&nbsp; [Discussions](https://github.com/vvvlladimir/Encrust/discussions)
&nbsp;·&nbsp; [Report a bug](https://github.com/vvvlladimir/Encrust/issues)

[![CI](https://github.com/vvvlladimir/Encrust/actions/workflows/ci.yml/badge.svg)](https://github.com/vvvlladimir/Encrust/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/vvvlladimir/Encrust?display_name=tag&color=brightgreen)](https://github.com/vvvlladimir/Encrust/releases/latest)
[![Status: alpha](https://img.shields.io/badge/status-alpha-orange.svg)](#where-it-stands)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux%20%7C%20Browser-lightgrey.svg)](#get-it)
[![License: AGPL v3](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)

</div>

---

> **Encrust is in alpha, and it is moving fast.** The whole pipeline works today — import,
> orient, hollow, support, slice, analyse, write and send to the printer — and it has printed real
> parts. It is also young, so keep a slicer you trust installed alongside it while it grows up.
> [Here is exactly where it stands.](#where-it-stands)

## What you get

- **A sliced file your printer accepts** — 100 printer profiles, 20-odd container formats, written
  and read back by the same code.
- **Supports that hold** — automatic branching trunks, manual placement, painted patches, rafts.
- **Hollowing that drains** — infill, drain holes, and a warning when resin would be trapped.
- **A layer-by-layer check before you print** — islands, suction cups and thin cross-sections
  found in what the file really cures, not in the model.
- **A command line for the same pipeline** — slice a folder of models without opening a window.

## Get it

**In a browser, with nothing to install:** open [**encrust.app/app**](https://encrust.app/app/).
It is the same Rust core compiled to WebAssembly and drawn with WebGPU; your model is opened and
sliced on your machine and nothing is uploaded. It needs a browser with WebGPU — a recent Chrome
or Edge is the safe bet — and it keeps working offline after the first visit. This browser build
is younger than the desktop one and still catching up to it; for a print you care about, use the
desktop app.

**On your desktop**, from [encrust.app/download](https://encrust.app/download/) or the
[latest release](https://github.com/vvvlladimir/Encrust/releases/latest):

| System | Download |
|---|---|
| Windows 10/11, 64-bit | `Encrust-windows-x86_64-setup.exe` |
| macOS 12+, Apple silicon | `Encrust-macos-aarch64.dmg` |
| macOS 12+, Intel | `Encrust-macos-x86_64.dmg` |
| Linux, x86-64 | `Encrust-linux-x86_64.AppImage` or `encrust_amd64.deb` |

Install it like any other application. You'll need a GPU with Vulkan, Metal or DirectX 12 — anything
from the last several years will do. Encrust can update itself from Settings › Updates once you turn
that on. The `encrust-*.tar.gz` and `.zip` archives beside the installers are portable copies, and
what that update downloads.

> **The builds aren't code-signed yet**, so the first launch needs one extra step. On Windows,
> choose *More info › Run anyway*. On macOS, drag Encrust to Applications, then run
> `xattr -dr com.apple.quarantine /Applications/Encrust.app` in Terminal.

## Your first print

1. Pick your printer and your resin — the printer from the list, the resin as one you make, since
   Encrust ships none (an exposure is measured on the machine in your room).
2. Load something small, orient it, add supports, hollow it if it is solid.
3. **Open the analysis view before you send it** — islands and suction cups are cheaper to fix here
   than on the plate.
4. Unless your printer is [the one verified below](#printers), print a calibration piece first.

Step by step: [encrust.app/guides](https://encrust.app/guides/).

## Printers

Encrust ships with profiles for **100 printers** from 14 makers — the full list, searchable, is at
[encrust.app/printers](https://encrust.app/printers/).

| Maker | Printers | Formats |
|---|---|---|
| Anycubic | 19 | `.pwmx` family, `.photon` |
| Elegoo | 16 | `.goo`, `.ctb` |
| Creality | 15 | `.cxdlp`, `.ctb` |
| Phrozen | 13 | `.ctb`, `.zip`, `.goo` |
| EPAX | 8 | `.ctb` |
| FlashForge | 8 | `.svgx` |
| Nova3D | 5 | `.cws` |
| Peopoly | 4 | `.ctb` |
| QIDI | 4 | `.ctb` |
| Wanhao | 3 | `.zip` |
| Prusa | 2 | `.sl1`, `.sl1s` |
| Kelant, UniFormation, Voxelab | 1 each | `.zip`, `.ctb`, `.svgx` |


Every other profile is built from the printer's published specification, and its files are
checked by reading them back — but nobody has cured resin with them yet. They are very
likely close, so start with a small calibration piece and check the build area and mirroring.
Encrypted `.ctb`, which some newer Chitu boards need, isn't supported yet.

**Own one of these, or a printer that isn't listed?** A single test print is the most valuable
thing you can give this project — see [Help it grow](#help-it-grow).

## What it can do

<details open>
<summary><b>Slicing and exposure</b></summary>

- Contour slicing with closed-contour guarantees, adaptive layer height by curvature, and several
  cutting planes per layer
- Exposure by height, anti-aliasing with grey levels and blur
- Textures pressed into the surface as relief, using UVs from OBJ and 3MF

</details>

<details>
<summary><b>Supports</b></summary>

- Manual and automatic placement, branching trunks, rafts and bracing
- Supports that stay out of the model and actually hold what they're under
- Painted patches, per-part editing, and contact tuning

</details>

<details>
<summary><b>Hollowing and mesh tools</b></summary>

- Hollowing with infill, drain holes and trapped-resin detection
- Diagnosis and repair: welding, holes, non-manifold edges, degenerate faces, winding
- Cut and split, auto-orientation, auto-arrange, multi-plate layouts, batch preparation

</details>

<details>
<summary><b>Print analysis</b></summary>

- What the written file really cures, layer by layer, and where it is likely to fail
- Island, suction and cross-section reports, with the measurements that produced them

</details>

<details>
<summary><b>Files and printers</b></summary>

- Reads STL, OBJ and 3MF, with UVs and one texture per material
- Writes `.goo`, `.ctb` v4/v5, `.cbddlp`, `.photon`, `.pwmx` and sixteen more Anycubic
  extensions, `.sl1`, `.sl1s`, the `.zip` of greyscale PNGs, `.cxdlp` v3/v4, `.svgx` and `.cws` —
  and reads every one of them back
- Sends over your local network: SDCP v3 and v1 (Elegoo and Chitu boards), and PrusaLink
- Projects and printer, resin and support profiles are plain TOML files you can open and edit

</details>

## From the command line

The same pipeline without a window, for batches and scripts:

```sh
encrust slice model.stl --printer elegoo-saturn-4-ultra --resin my-grey --center -o model.goo
```

Every flag, with worked examples, is in [`docs/cli.md`](docs/cli.md).

<details>
<summary><b>Building from source</b></summary>

With the toolchain `rust-toolchain.toml` pins:

```sh
git clone https://github.com/vvvlladimir/Encrust.git
cd Encrust
cargo run --release -p encrust-app --bin encrust-gui   # the window
cargo run --release -p encrust-cli --bin encrust -- --help
```

[CONTRIBUTING.md](CONTRIBUTING.md#getting-set-up) covers the full development setup, and
[`docs/design/web-build.md`](docs/design/web-build.md) the browser build.

</details>

## Where it stands

**Alpha.** Every stage of the pipeline is in place and covered by around 1,500 unit, property and
integration tests, which run on Windows, macOS and Linux for every change.

Being honest about what alpha means here:

- **One printer is proven.** Of the hundred profiles, only the ELEGOO Saturn 4 Ultra has printed a
  file Encrust wrote. The rest are very likely close, but unconfirmed.
- **Resin settings are a starting point.** Exposure and lift values in the bundled profiles aren't
  calibrated for your bottle — run a calibration print first.
- **The browser build trails the desktop one.** It runs the same core, and it is newer.
- **There will be bugs.** Encrust hasn't yet had the thousands of hours of real-world use that
  shake out the last ones. Before a long print, it's worth comparing its output with a slicer you
  already trust.

The worst case is a failed print: a few hours and a few millilitres of resin. If that sounds like a
fair trade for trying something new, you're exactly who this release is for — and every report
you send makes the next one better.

## Help it grow

Encrust is actively developed, and right now the people who try it shape it the most.

- **Test your printer.** Tell us which printer you have, attach a file its own software
  produced, and say whether Encrust's file printed. This is the one thing the project can't do on
  its own. [Open an issue](https://github.com/vvvlladimir/Encrust/issues/new/choose).
- **Report what breaks.** A bug report or a mesh that trips it up is genuinely useful.
- **Say what you'd want.** Ideas and questions are welcome in
  [Discussions](https://github.com/vvvlladimir/Encrust/discussions).
- **Send code.** Start with [CONTRIBUTING.md](CONTRIBUTING.md) and [`AGENTS.md`](AGENTS.md);
  [`docs/decisions/`](docs/decisions/) explains the reasoning behind every structural choice.

If Encrust looks useful to you, a ⭐ helps other resin printers find it.

## Licence

[AGPL-3.0-only](LICENSE). You're free to run, study, change and share Encrust; if you distribute a
modified version, or offer it to others over a network, you share its source too.

## A word of care

Encrust prepares files for machines that cure resin with ultraviolet light. It can't know whether
your settings suit your resin, printer or model, and a wrong exposure or lift value can ruin a
print and, occasionally, damage hardware. Check what it produces before a long job, and follow your
printer's and resin's own instructions. Provided without warranty of any kind, as the licence says.

<div align="center">

<img src="assets/icon/encrust.svg" alt="" width="96" height="96">

# Encrust

### A resin slicer that lives on your computer and answers to you.

Free and open source, for MSLA and SLA printers.

[**Download**](https://github.com/vvvlladimir/Encrust/releases/latest)
&nbsp;·&nbsp; [What's new](CHANGELOG.md)
&nbsp;·&nbsp; [Discussions](https://github.com/vvvlladimir/Encrust/discussions)
&nbsp;·&nbsp; [Report a bug](https://github.com/vvvlladimir/Encrust/issues)

[![CI](https://github.com/vvvlladimir/Encrust/actions/workflows/ci.yml/badge.svg)](https://github.com/vvvlladimir/Encrust/actions/workflows/ci.yml)
[![Status: alpha](https://img.shields.io/badge/status-alpha-orange.svg)](#where-it-stands)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](#try-it)
[![License: AGPL v3](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)

</div>

---

> **Encrust is in alpha, and it is moving fast.** The whole pipeline works today — import,
> orient, hollow, support, slice, analyse, write and send to the printer — and it has printed real
> parts. It is also young, so keep a slicer you trust installed alongside it while it grows up.
> [Here is exactly where it stands.](#where-it-stands)

## Try it

Grab the build for your system from the
[**latest release**](https://github.com/vvvlladimir/Encrust/releases/latest):

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

**A good first print:** pick your printer and resin, load something small, and look at the
analysis view before you send it. Unless your printer is the one verified below, start with a
calibration piece rather than the model you care about.

<details>
<summary><b>Prefer the command line, or building from source?</b></summary>

With a recent stable Rust toolchain (`rust-toolchain.toml` pins the version):

```sh
git clone https://github.com/vvvlladimir/Encrust.git
cd Encrust
cargo run --release -p encrust-app --bin encrust-gui
```

Slicing without a window:

```sh
cargo run --release -p encrust-cli --bin encrust -- slice model.stl \
  --printer elegoo-saturn-4-ultra --resin my-grey --center -o model.goo
```

Every flag is described in [`docs/cli.md`](docs/cli.md), and
[CONTRIBUTING.md](CONTRIBUTING.md#getting-set-up) covers the full development setup.

</details>

## Printers

Encrust ships with profiles for **100 printers** from 14 makers. It ships no resin: an
exposure is measured on the machine in your room, so the first resin is one you make, in
the window or as a TOML file.

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

**So far, only one has actually printed a file Encrust wrote: the ELEGOO Saturn 4 Ultra.**
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

## Where it stands

**Alpha.** Every stage of the pipeline is in place and covered by around 1,500 unit, property and
integration tests, which run on Windows, macOS and Linux for every change.

Being honest about what alpha means here:

- **One printer is proven.** Of the hundred profiles, only the ELEGOO Saturn 4 Ultra has printed a
  file Encrust wrote. The rest are very likely close, but unconfirmed.
- **Resin settings are a starting point.** Exposure and lift values in the bundled profiles aren't
  calibrated for your bottle — run a calibration print first.
- **There will be bugs.** Encrust hasn't yet had the thousands of hours of real-world use that
  shake out the last ones. Before a long print, it's worth comparing its output with a slicer you
  already trust.

The worst case is a failed print: a few hours and a few millilitres of resin. If that sounds like a
fair trade for trying something new, you're exactly who this release is for — and every report
you send makes the next one better.

## Your data stays yours

These are the only network connections Encrust ever makes:

| What | Where | When |
|---|---|---|
| Sending a sliced file | Your printer's IP on your own network (SDCP or PrusaLink) | Only when you press *Send* |
| Finding printers | A UDP broadcast on your local network | Only when you ask it to look |
| Checking for updates | `github.com`, over HTTPS | Off by default; daily once enabled, or when you press *Check now* |
| Downloading an update | `github.com`, the archive for your platform | Only when you press *Install and restart* |

An update check sends nothing but the request itself, and an update is installed only if it
carries the project's own signature. No analytics, no crash reporting, no licence checks, no
account, no cloud. Your models, profiles and projects are just files on your disk.

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

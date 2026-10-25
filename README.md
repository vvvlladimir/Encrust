<div align="center">

<img src="assets/icon/encrust.svg" alt="" width="96" height="96">

# Encrust

### The fast resin slicer that checks every layer.

Encrust is a free, open-source resin slicer that checks every layer of the file it writes for
islands, suction cups and thin cross-sections before you print. Windows, macOS, Linux and browser.
Supports, hollowing and 20-odd printer formats included — all of it on your own computer, with no
account and no cloud.

[**Download**](https://encrust.app/download/)
&nbsp;·&nbsp; [**Try it in a browser**](https://encrust.app/app/)
&nbsp;·&nbsp; [Website](https://encrust.app)
&nbsp;·&nbsp; [Guides](https://encrust.app/guides/)
&nbsp;·&nbsp; [Printers](https://encrust.app/printers/)
&nbsp;·&nbsp; [What's new](CHANGELOG.md)
&nbsp;·&nbsp; [Discussions](https://github.com/vvvlladimir/Encrust/discussions)
&nbsp;·&nbsp; [Report a bug](https://github.com/vvvlladimir/Encrust/issues)

[![Latest release](https://img.shields.io/github/v/release/vvvlladimir/Encrust?display_name=tag&color=brightgreen)](https://github.com/vvvlladimir/Encrust/releases/latest)
[![Status: alpha](https://img.shields.io/badge/status-alpha-orange.svg)](CHANGELOG.md)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux%20%7C%20Browser-lightgrey.svg)](#get-it)
[![License: AGPL v3](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)

</div>

---

## Get it

| System | Download |
|---|---|
| Browser, nothing to install | [![Open in browser](https://img.shields.io/badge/Open-encrust.app%2Fapp-2ea44f?style=for-the-badge)](https://encrust.app/app/) |
| Windows 10/11, 64-bit | [![Microsoft Store](https://img.shields.io/badge/Microsoft_Store-Get_it-0078D4?style=for-the-badge)](https://apps.microsoft.com/detail/9NB0ZMB3M8H8) [![Installer](https://img.shields.io/badge/Installer-.exe-0078D4?style=for-the-badge)](https://github.com/vvvlladimir/Encrust/releases/latest/download/Encrust-windows-x86_64-setup.exe) |
| macOS 12+, Apple silicon | [![macOS Apple silicon](https://img.shields.io/badge/Download-.dmg-555555?style=for-the-badge&logo=apple)](https://github.com/vvvlladimir/Encrust/releases/latest/download/Encrust-macos-aarch64.dmg) |
| macOS 12+, Intel | [![macOS Intel](https://img.shields.io/badge/Download-.dmg-555555?style=for-the-badge&logo=apple)](https://github.com/vvvlladimir/Encrust/releases/latest/download/Encrust-macos-x86_64.dmg) |
| Linux, x86-64 | [![AppImage](https://img.shields.io/badge/Download-AppImage-E95420?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/vvvlladimir/Encrust/releases/latest/download/Encrust-linux-x86_64.AppImage) [![deb](https://img.shields.io/badge/Download-.deb-E95420?style=for-the-badge&logo=debian&logoColor=white)](https://github.com/vvvlladimir/Encrust/releases/latest/download/encrust_amd64.deb) |

> [!NOTE]
> **First launch needs one extra step.** The downloaded builds aren't code-signed yet, so the
> system warns about them. The Microsoft Store copy is signed and needs nothing.
> - **Windows:** on the blue warning, click *More info › Run anyway*.
> - **macOS:** move Encrust to Applications, then run once in Terminal:
>   `xattr -dr com.apple.quarantine /Applications/Encrust.app`


Every file of every version: [releases](https://github.com/vvvlladimir/Encrust/releases).

## Features

**Check before you print.** Encrust reads back the file it wrote and walks every layer of what
will really cure, after supports and hollowing:
- islands, suction cups, thin sections and weak necks, each with its layer and measurement;
- the layers that pull the film hardest, the widest step and the levers;
- one press to support every island, remove them, or drill the suction cups;
- print time, resin volume, weight and cost, counted from what actually cures.

**Supports.** Automatic branching supports that go only under what hangs, stay out of the
model and hold each island where it begins; manual placement, painted patches, rafts, bracing
and support profiles you can save.

**Hollowing and drainage.** Hollow inward, outward (a mould) or through the floor, with infill
lattices, round or polygon drain holes, and a warning when resin would stay trapped.

**Model preparation.** STL, OBJ and 3MF import with mesh repair (holes, non-manifold edges,
flipped faces); auto-orient, auto-arrange, arrays and several plates per project; cut with
dowels or other joints; textures and text pressed into the surface, and models combined.

**Slicing and exposure.** Adaptive layer height, anti-aliasing with grey levels and blur,
exposure by height, shrinkage compensation and wall tolerance.

**Files and printers.** Writes `.goo`, `.ctb` v4/v5, `.cbddlp`, `.photon`, the `.pwmx` family,
`.sl1`, `.sl1s`, `.cxdlp`, `.svgx`, `.cws` and the `.zip` of PNGs — and reads each one back,
including files from other software. Sends, starts and watches prints over the network with
SDCP (Elegoo and Chitu boards) and PrusaLink.

**Runs where you are.** Desktop on Windows, macOS and Linux; the same core in the browser via
WebAssembly and WebGPU, offline and with nothing uploaded; a command line for scripts and
batches. Projects and profiles are plain TOML.

Everything, with what it cannot do yet: [`docs/features.md`](docs/features.md).

## Printers

Profiles for Anycubic, Elegoo, Creality, Phrozen, EPAX, FlashForge, Nova3D, Peopoly, QIDI,
Wanhao, Prusa, Kelant, UniFormation and Voxelab — the full list is at
[encrust.app/printers](https://encrust.app/printers/). Encrust ships no resin: an exposure is
measured on your machine, so start with a calibration piece.

## From the command line

```sh
encrust slice model.stl --printer elegoo-saturn-4-ultra --resin my-grey --center -o model.goo
```

Every flag is in [`docs/cli.md`](docs/cli.md).

<details>
<summary><b>Building from source</b></summary>

```sh
git clone https://github.com/vvvlladimir/Encrust.git
cd Encrust
cargo run --release -p encrust-app --bin encrust-gui   # the window
cargo run --release -p encrust-cli --bin encrust -- --help
```

[CONTRIBUTING.md](CONTRIBUTING.md#getting-set-up) covers the setup,
[`docs/design/web-build.md`](docs/design/web-build.md) the browser build.

</details>

## Help it grow

- **Test your printer** and say whether Encrust's file printed —
  [open an issue](https://github.com/vvvlladimir/Encrust/issues/new/choose).
- **Report what breaks**, or ask in [Discussions](https://github.com/vvvlladimir/Encrust/discussions).
- **Send code:** start with [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

[AGPL-3.0-only](LICENSE). Encrust prepares files for machines that cure resin with UV light; a
wrong exposure or lift can ruin a print or damage hardware. Provided without warranty of any kind.

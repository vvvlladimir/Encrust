# 0177. The cores build for a browser: wasm32, one thread, no clock, a cavity budget

- **Status:** Accepted
- **Date:** 2026-10-03

## Context

The web version runs the whole pipeline in the page; a server only hosts files. The cores
already compiled for `wasm32-unknown-unknown`, but compiling is not running: there
`SystemTime::now` and `thread::spawn` panic and `std::fs` returns an error. Three places
reached for them — the sliced-file writers stamped their own creation time, and the mesh
loaders opened their file and its siblings by path. A wasm32 module addresses 4 GB;
Memory64 ships in two of the three browser engines and only as a preview in the third,
and runs slower where it exists. Threads in wasm still need a nightly compiler and
cross-origin isolation.

## Decision

- **Target.** `wasm32-unknown-unknown`, single-threaded, stable Rust. Rayon runs every
  parallel loop on the calling thread there with no change to the cores; threads are step
  A3. CI checks every crate but the two front ends, the `net-*` crates and `xtask` for it.
- **No clock in a core.** `PrintJob::created_unix_s` is the time a file is stamped with,
  filled by the caller through `Plate::created_unix_s`; `core-format` only formats it.
- **No path in a loader.** `MeshLoader::read` takes a `ModelFile`: a name, a seekable
  source and a closure handing over a file beside it by name. `load(&Path)` is a provided
  method over the disk.
- **Memory.** wasm32 and its 4 GB, held to by the cavity budget `core-volume` already has
  (ADR 0063): the browser asks for 1 GB, and a finer lattice is coarsened to fit.
- **The front end** is `web-engine`, on `core-engine` and `wasm-bindgen`: the bytes of a
  `.encrust` project in (ADR 0178), the bytes of a sliced file out, the time passed in
  from JavaScript.

## Consequences

Measured on three real models of about a million faces each (`docs/design/web-build.md`):
one wasm thread under Node takes 1.3 to 1.6 times one native thread, and the worst plate,
a hollowed bust writing 275 MB, peaked at 1.1 GB of wasm memory. The 4 GB ceiling is not
what limits the web build; the finished file held in memory is, which step A4 streams to
OPFS. A core that reads a clock, spawns a thread or opens a path breaks the browser at run
time and not in CI — review is what keeps them out. Reopen when Memory64 ships in every
browser engine or a plate is measured past 3 GB.

## Alternatives considered

### Memory64 now, as a second build

It lifts 4 GB to 16 GB, but needs nightly and `build-std`, is missing from one shipping
browser engine and costs speed on every pointer. Nothing measured comes near the ceiling it lifts.

### A clock crate in `core-format`

One `cfg` and `web-time` would keep the writers stamping themselves. It adds a dependency
to a format crate to keep a side effect that a test cannot pin; the caller already has a
clock.

### The option that won, and what it costs

Every front end now reads the clock itself, and each `PrintJob` literal names a time. A
plate whose cavity needs more than 1 GB comes out coarser in a browser than at the desk.

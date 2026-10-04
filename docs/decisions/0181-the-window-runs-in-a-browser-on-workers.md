# 0181. The window runs in a browser, on workers sharing its memory

- **Status:** Accepted
- **Date:** 2026-10-04

## Context

The cores already ran in a page on one thread (ADR 0177); the window did not. It starts a
thread for every job, writes files by path, reads its profiles and preferences from a
directory, and asks native dialogs for names. In a browser `thread::spawn` and
`SystemTime::now` panic, `std::fs` fails, the page's own thread may never wait — a contended
lock or a parallel loop there traps — and a worker gets a file in private storage only by a
promise. Threads in wasm still need a nightly compiler rebuilding the standard library and
a page isolated by COOP and COEP. Under them a GPU object is not `Send`, so it cannot sit in
egui's paint-callback map, and eframe's web painter gives the viewport no stencil plane.

## Decision

- **A front end of its own.** `encrust-web` is a `cdylib` holding only `start(canvas,
  bindings)`, as `main.rs` is the desk's; the window stays one crate, with `cfg` only where
  a browser differs.
- **Threads are workers.** Every thread is a worker the page starts on the module's shared
  memory; its task crosses over a Rust channel, and a worker that needs another asks the
  page, since a worker's child starts only when the worker yields. `job::spawn` is
  `std::thread::spawn` at the desk and this in a browser. Rayon's global pool is built of
  such workers by the first job, a worker, because building it waits; every job shares it,
  held to `cores − 1`. The page's thread is a pool of one, so a parallel loop there runs
  in place.
- **Nightly for the browser only.** `cargo xtask web` builds with `-Z build-std` and the
  atomics flags into `target/web/`; the workspace stays on stable and CI checks the
  threaded build on main. A page that is not isolated gets the headers from a service
  worker of ours, and failing that says so; there is no single-threaded window.
- **Files.** What the user hands over is a `files::Handed`: a path, or bytes with the files
  picked beside them. A dialog's answer arrives on a later frame. A sliced file is written
  on its worker to private storage through a synchronous handle, buffered, and handed to the
  page as a download; a browser without that storage gets it written in memory.
- **Settings.** Preferences are one `localStorage` entry. `printer-profiles` gains
  `ProfileStore`, which the catalogue writes through: `DirStore` at the desk, the page's
  storage in a browser.
- **What is left out.** The printer network and updates are hidden; the cut is drawn
  uncapped, the capping pipelines being built only where the depth buffer has a stencil.

## Consequences

The window opens, imports, previews, slices and saves a project in Chromium and WebKit.
A million-face plate slices in 1.2 s on seven workers, where seven native threads take
0.7 s (`docs/design/web-build.md`). Every thread costs a worker's start and its stack, and
a panic in one ends that worker without an outcome, so the job never finishes. The page's
thread runs a parallel loop alone, which makes opening a large project slower than at the
desk. Reopen when eframe's web painter
asks for a stencil plane, when wasm threads reach stable Rust, or when a browser drops
synchronous private-storage handles.

## Alternatives considered

### `wasm-bindgen-rayon`

It builds the pool from the page and has no answer for a job's own thread, nor for a task
that awaits a promise first; the window needed both, and both are the same thirty lines.

### One worker running the jobs, no shared memory

Stable Rust and no isolation headers, but every mesh and scene would be copied through
`postMessage`, and the work would run on one core: 1.3 to 1.6 times slower than one native
thread (ADR 0177), and the window rewritten around messages.

### The option that won, and what it costs

A nightly toolchain to keep building, a page that has to be served with two headers, and
the threading model of the window living in a few hundred lines of our own, with a
JavaScript file inside a Rust crate.

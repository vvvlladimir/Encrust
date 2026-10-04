# The browser build

Two things run in a page. `encrust-web` is the whole window, on threads (ADR 0181).
`web-engine` is the pipeline alone, a `.encrust` project in and a sliced file out, on one
thread with no file system and no clock (ADR 0177). This page is how each is built, run and
measured.

## The window

```sh
rustup toolchain install nightly --component rust-src
rustup target add wasm32-unknown-unknown --toolchain nightly
cargo install wasm-bindgen-cli --version <the wasm-bindgen in Cargo.lock> --locked
cargo xtask web            # into target/web/dist; --no-opt skips wasm-opt, --check only compiles
```

Threads in wasm rebuild the standard library with atomics, which only nightly does; the
flags are in `crates/xtask/src/web.rs` and build into `target/web/` so that they never
touch the stable builds. Serve `target/web/dist` with `Cross-Origin-Opener-Policy:
same-origin` and `Cross-Origin-Embedder-Policy: require-corp`; `_headers` says so to a host
that reads it. Without them the page registers `isolate.js`, a service worker that adds the
headers, and reloads once; where even that fails it says what the host must send.

### How it runs

- **Threads.** `web/thread.js` is both the page's way to start a worker and the worker:
  it imports the module's script, starts it on the shared memory with a 2 MiB stack, takes
  one task off the Rust channel in `web/thread.rs`, runs it and frees its stack. Only the
  page starts workers: Chromium starts a worker's child only once that worker returns to
  its event loop, which one blocked building a pool never does. The first job builds
  rayon's global pool; the page's thread is a pool of one with `use_current_thread`, so
  nothing it runs in parallel ever waits.
- **Never block the page's thread.** No contended lock, no blocking `recv`, no parallel
  loop expecting help there: wasm traps on `Atomics.wait` on that thread. Jobs report over
  channels the window drains with `try_recv`, as at the desk.
- **GPU objects** are not `Send` with atomics, so the viewport keeps its resources in a
  thread-local of the page's thread instead of egui's callback map (`render.rs`).
- **Files.** A sliced file is written on its worker to private storage through a
  `FileSystemSyncAccessHandle`, a megabyte per call: unbuffered, a million-face plate took
  36 s instead of 1.2 s. The worker hands the file to the page, which alone may start a
  download. A browser without private storage, such as a private window, gets it written
  in memory.
- **Storage.** Preferences are the `localStorage` entry `encrust.preferences`, and each of
  the user's profiles one entry under `encrust.profiles/`.

### What a browser does not have yet

- The printer network: a sliced file is downloaded, and the Network card says so (ADR 0182).
- Updates; the Updates page is hidden.
- A capped section: eframe's web painter makes its depth buffer without a stencil plane
  whatever it is asked for, so the cut is drawn open.
- A warning before the tab is closed over unsaved work.
- A worker that panics ends without an outcome, so its job never finishes.

### Measured, 2026-10-04

A 998 000-face sphere of radius 30 mm on a Mars 4 Ultra panel, 0.05 mm layers, 8 cores:
1.2 s from the Slice button to the download in headless Chromium on seven workers, against
1.24 s for one native thread and 0.72 s for seven. The window's `.wasm` is 13 MB, 3.7 MB
under brotli.

## The pipeline alone

### Building

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version <the wasm-bindgen in Cargo.lock> --locked
# and wasm-opt, from binaryen, through the system's package manager

cargo build --release -p web-engine --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir crates/web-engine/www/pkg \
  target/wasm32-unknown-unknown/release/web_engine.wasm
wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
  --enable-mutable-globals --enable-reference-types --enable-multivalue \
  crates/web-engine/www/pkg/web_engine_bg.wasm -o crates/web-engine/www/pkg/web_engine_bg.wasm
```

`wasm-bindgen-cli` must be the exact version of the crate. `wasm-opt` is told the features
rustc already emits, because it refuses what it was not told about. `-O3` and `-Oz` come
out within 2 % of each other, so the build optimises for speed.

### Running

`www/index.html` takes a project and downloads the file; the work runs in `www/worker.js`
because a page's own thread may not block. Serve `www/` over HTTP — a module does not load
from `file://` — for instance `python3 -m http.server -d crates/web-engine/www`.

### Measuring

The same `slice_project` runs natively and in wasm, so the two numbers are one run on two
targets:

```sh
RAYON_NUM_THREADS=1 cargo run --release -p web-engine --example measure -- plate.encrust 1024
node crates/web-engine/www/measure.mjs crates/web-engine/www/pkg plate.encrust 1024
```

Natively, peak memory is the live bytes the allocator counts. In wasm it is the linear
memory after the run, which only grows, so it is the run's peak plus what the allocator
could not reuse.

### Measured, 2026-10-03

Saturn 4 Ultra panel (11520 x 5120), 0.05 mm layers, supports grown from automatic points,
a 2 mm wall where hollowed, a 1 GB cavity budget. 8 cores, Node 24.

| Plate | Faces | File | Native, 1 thread | Native, 8 | Wasm, 1 | Native live | Wasm memory |
|---|---|---|---|---|---|---|---|
| Figure, 32 mm | 0.97 M | 6.7 MB | 0.7 s | 0.4 s | 1.0 s | 105 MB | 126 MB |
| the same, hollowed | | 7.3 MB | 2.6 s | 1.1 s | 4.1 s | 184 MB | 257 MB |
| Bust, 85 mm | 1.01 M | 177 MB | 5.6 s | 2.2 s | 7.4 s | 305 MB | 596 MB |
| the same, hollowed | | 275 MB | 27.4 s | 6.3 s | 34.6 s | 575 MB | 1138 MB |
| Stand, 85 mm | 0.87 M | 188 MB | 4.9 s | 2.1 s | 6.8 s | 308 MB | 587 MB |
| the same, hollowed | | 253 MB | 12.9 s | 3.8 s | 20.8 s | 313 MB | 665 MB |

One wasm thread is 1.3 to 1.6 times one native thread. Wasm memory runs to twice the native
peak on the plates with a large file, which are the plates whose output is a growing
`Vec` in the same memory; streaming the file out (step A4) is what lowers it. The
`.wasm` is 1.5 MB, 425 KB under brotli.

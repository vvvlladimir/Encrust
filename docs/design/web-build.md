# The browser build

`web-engine` is the pipeline as a browser runs it: a `.encrust` project in, a sliced file
out, on one thread with no file system and no clock (ADR 0177). This page is how it is
built, run and measured.

## Building

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

## Running

`www/index.html` takes a project and downloads the file; the work runs in `www/worker.js`
because a page's own thread may not block. Serve `www/` over HTTP — a module does not load
from `file://` — for instance `python3 -m http.server -d crates/web-engine/www`.

## Measuring

The same `slice_project` runs natively and in wasm, so the two numbers are one run on two
targets:

```sh
RAYON_NUM_THREADS=1 cargo run --release -p web-engine --example measure -- plate.encrust 1024
node crates/web-engine/www/measure.mjs crates/web-engine/www/pkg plate.encrust 1024
```

Natively, peak memory is the live bytes the allocator counts. In wasm it is the linear
memory after the run, which only grows, so it is the run's peak plus what the allocator
could not reuse.

## Measured, 2026-10-03

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

# 0037. Optimise the dev profile

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

`cargo run -p encrust-app --bin encrust` is how the window is started, in the README, in
`AGENTS.md` and in every day of work on it. Without `--release` that is an unoptimised
build of every crate in the graph, `glam` and `rayon` included.

Measured on a 880 604-triangle model, the whole command-line pipeline — load, weld,
orient, diagnose, slice, rasterise and write a 69 MB `.goo` file — takes 2.74 s in release
and 23.95 s unoptimised. Nine times. Importing that model in the window therefore looks
like the application has hung, and the slicer is judged on work it is not actually doing.

Nothing about this project is helped by unoptimised numeric code. The debug information is
what a debugger and a backtrace need; the missing inlining is not.

## Decision

The workspace sets `[profile.dev] opt-level = 1` and `[profile.dev.package."*"]
opt-level = 3`. Our own crates keep enough of their shape to step through; every
dependency, which carries no debugging work of ours, is built at full speed. Debug
assertions, overflow checks and debug info stay on, so a debug build is still a debug
build.

## Consequences

The same pipeline now takes 3.49 s unoptimised instead of 23.95 s. The window no longer
looks hung on an ordinary `cargo run`.

Dependencies are compiled once at `opt-level = 3` and then cached, so the cost lands on
the first build after a dependency changes rather than on each iteration. A clean debug
build of this workspace went from about 30 s to about 50 s.

Timings quoted anywhere in the documentation stay release timings. A debug build is faster
than it was, not as fast as release, and a benchmark is still `cargo bench`.

## Alternatives considered

### Leave it, and tell people to pass `--release`

It is one flag, and it is already what the benchmark commands use. It also loses every
debug assertion and every readable backtrace in the build people actually run, and relies
on remembering it. The measurements that made this an issue were taken by forgetting it.

### `opt-level = 3` for our own crates too

Fastest, and the numbers would nearly match release. Stepping through optimised code is
unpleasant enough that the debug build would stop being useful for debugging, which is the
one thing it is for. `opt-level = 1` keeps that and recovers most of the speed, because
the hot loops are in `glam` and in code the optimiser handles well at level 1.

### The option that won, and what it costs

Builds are slower, the first one after a dependency bump noticeably so, and the dev build
no longer matches what a plain `cargo build` produces in other Rust projects — which is a
small surprise for anyone reading a profile or a backtrace and expecting no inlining at
all.

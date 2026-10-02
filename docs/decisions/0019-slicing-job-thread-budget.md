# 0019. A background job gets its own rayon pool, one thread short of the machine

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

ADR 0018 put slicing on a worker thread so the window keeps drawing. It did not say how
much of the machine that worker may take. Both stages under it are parallel: the slicer
splits layers across rayon, and the rasteriser runs a window of layers at a time on the
same pool. Left alone, both use rayon's global pool, which is sized to every core the
machine has.

On a laptop that is the whole machine. The window competes with the job for the last core,
the fans spin up, and a slicer that is supposed to be a light desktop tool behaves like a
render farm for as long as the job lasts.

The CLI is a different case. It is the whole of what the user asked to run, it has no
frame to draw, and taking every core is exactly what someone who typed `slice` wants.

## Decision

`encrust-app` builds a `rayon::ThreadPool` per job with
`available_parallelism().saturating_sub(1)` threads, never fewer than one, and runs the
whole pipeline inside `pool.install`. That covers the slicer's own parallelism as well as
the rasteriser's, because `install` makes the pool the current one for everything the
worker calls.

The raster window keeps matching the thread count, so peak memory is still one mask per
thread, as ADR 0010 requires.

`encrust-cli` is untouched and keeps the global pool and all cores.

## Consequences

- The window keeps a core to draw with, so the progress bar and the camera stay smooth
  while a job runs, and the machine stays usable for anything else.
- A job is slower than it could be by roughly one core's share: an eighth on eight cores,
  a quarter on four. On a two-core machine that is half the throughput, which is the worst
  case this rule has.
- Peak memory drops by one mask alongside it, since the window follows the thread count.
- The pool is built per job and dropped with it, so an idle window holds no threads. The
  cost is one pool construction per Slice press, which is nothing next to the job.
- The thread count is not a setting. If someone wants the machine pinned, they have the
  CLI; the signal to add a setting is a user who wants the opposite on a many-core
  workstation.

## Alternatives considered

### Use the global pool, like the CLI

Simplest, and fastest for the job in isolation. Rejected because the window is not in
isolation: it has to draw at the same time, and the frame it drops is the one showing the
progress of the job that dropped it.

### Configure the global pool once at startup

`ThreadPoolBuilder::build_global` would apply the same budget without a pool per job. It
can only be called once per process and it would silently apply to anything else in the
window that ever reaches for rayon, which makes a startup-time decision bind code that has
not been written yet. A pool that belongs to the job says what it is for.

### Lower the worker thread's priority instead

The honest way to say "use everything, but yield to the window". It needs per-platform
calls, has no portable API in std, and on macOS quality-of-service classes interact with
rayon's own threads in ways that would have to be tested on every platform the project
builds for. Not worth it for the one core this buys back.

### The option that won, and what it costs

Taking a core away is a blunt instrument. It does not distinguish a four-core laptop from
a sixty-four-core workstation, where one core is noise and the rule is pointless; it does
not adapt when the window is idle and could happily give the job everything; and it does
nothing about memory bandwidth, which is what the rasteriser actually saturates — the
spare core does not mean the machine feels idle, only that it is not queued. A real answer
would measure frame times and scale the pool to keep them under a budget. That is a
feedback loop to maintain, and this is a constant.

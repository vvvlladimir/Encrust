# 0100. A batch is the single run, once per model, in the same binary

- **Status:** Accepted
- **Date:** 2026-09-26

## Context

A print farm wants a folder sliced overnight: every part oriented, hollowed, supported,
cut and written, with something machine-readable to gate on in the morning.

The `slice` binary already has thirty flags that all apply to a model — the printer, the
resin, hollowing, exposure bands, adaptive layers, orientation. A separate batch binary
would have to declare every one of them again or share a struct across two crate roots.

## Decision

`slice` takes a directory where it takes a file, and that is the batch. Every flag means
what it already meant, applied to each model in turn. The output becomes a directory:
one sliced file per model, named after it, with `<model>.json` beside it and `batch.json`
over the lot.

One model is one job. Orienting and centring is all the "arranging" a batch does, because
what a farm wants is one file per part, not several parts on one plate — that is what the
window's plates are for.

The per-model pipeline moved out of `main.rs` into `pipeline::slice_one`, which both the
single run and the batch call. It prints as it goes for the single run and says nothing
for the batch, but the work and the order are one piece of code.

Models are cut one at a time by default (`--jobs 1`). A failed model is reported and the
run carries on; `--strict` turns any failure or unclean model into a non-zero exit.

Supports come to the CLI with this: the model is sliced once to find where it needs
holding, the trees are meshed and merged into it, and the merged mesh is sliced again for
the file.

## Consequences

Anything the single run can do, the batch does, the day it is added — there is no second
argument surface to keep in step.

Supporting a model costs two slicing passes. It is the same two the window pays, and the
first one is over the model alone, but a batch with `--supports` is roughly twice the
work of one without.

`--jobs 1` leaves throughput on the table for a folder of small parts, where the inner
parallelism cannot fill the cores. It is the safe default because each model in flight
holds its own stack: raising it multiplies peak memory by the number of jobs, and the
machine that runs out is the one running overnight unattended.

A directory input is a mode switch with no flag on it. It is unambiguous — a directory is
not a mesh — but it does mean `slice some/dir` does something quite different from
`slice some/model.stl`, and nothing in the argument list says so except the help text.

Reopen `--jobs` if a real farm's numbers say the inner parallelism is idle; the measurement
is a batch of small parts against one of large ones.

## Alternatives considered

### A second binary, `batch`

Clean separation, and its own help text. It cannot see `main.rs`'s `Args`, so every flag
would be declared twice and drift apart on the first one added to only one of them.

### A `--batch <DIR>` flag

Explicit, which is worth something. It makes `input` mean nothing in batch mode and adds a
flag combination to validate, for a distinction the argument's own type already carries.

### Packing the whole directory onto one plate

Closer to what the window does, and it is what "arranges" suggested. It produces one file
and one report for a folder, which is the opposite of what a farm queueing parts wants,
and it makes the per-model report the step asked for impossible.

### The option that won, and what it costs

The single run now goes through a function built to serve two callers, so its printing is
behind a `talk` flag rather than being simply where the work is. Anyone reading
`main.rs` to find out what `slice` does now finds argument parsing and a dispatch, and has
to open `pipeline.rs` for the answer.

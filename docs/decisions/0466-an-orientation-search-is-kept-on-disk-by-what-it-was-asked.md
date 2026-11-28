# 0466. Keep an orientation search on disk by the hash of what it was asked

- **Status:** Accepted
- **Date:** 2026-11-27

## Context

A chat client's `compare_poses` runs the command line once for the numbers and once a picture,
and `set_pose` once more for the pose taken: each run searches again. On a 1.4-million-face
statue cut in two, a search costs about 6.5 s of a run's 8 s, and staging and drawing the rest:
one compare and its pose took about 51 s, of which 45 s were the same search six times. Drawing
the pictures side by side saved 6 s of 32 for four times the memory. The search,
`orient_for_print`, is a function of its arguments alone; the pick only indexes what it ranks.

## Decision

- **`core_engine::orient_for_print_kept`** reads the ranked choices from a folder, under the
  SHA-256 of every argument that decides it — the outer, shell and cut meshes byte by byte, the
  vents, the faces painted as shown, the settings and the trial's printer, resin, support plan
  and places kept clear, by their `Debug` text — and of the engine's version; the raster window
  only batches layers, and four threads or eight rank alike, or searches and keeps
  them there. A file is written beside its place and renamed onto it; the least recently read go
  past 512. A folder that cannot be read or written is a search as before. A search handed what
  shows ready is never kept.
- **A `Document` searches through it** where its caller names a folder (`keep_poses_in`), and
  so does the window's Orient, which carries out a client's orient edits as well.
- **One folder a build, the window's and the command line's alike** (`kept_poses_folder`):
  `poses/v1/<version>-<build>` under the user's cache, `<build>` the `ENCRUST_BUILD_ID` the
  engine was compiled with — the commit in a release, the commit and a hash of the changes from
  `cargo xtask app` and `mcpb` — else when the program was made, a folder of its own. The four
  latest other builds' folders stay. `ENCRUST_POSE_CACHE` names another folder, or `off`; a
  debug build keeps none.
- **Choices are JSON**, `Choice`, `Oriented`, `Score`, `Printout` and `Ruling` deriving serde,
  and the engine's JSON reads a float back to the same bits (`float_roundtrip`).

## Consequences

A compare of four pictures and its pose cost one search: about 15 s on that statue, not 51,
the pose 2.6 s, not 10. Every argument added to the search must reach the key, or a kept answer
outlives a change it should not; `Debug` of the settings and the trial carries a new field by
itself, a new argument of the function does not. A field of the report the search does not
decide still differs from run to run, as before. A pose a client asks of the window is read, not
searched, when the window and the server's command line were built together and the window
builds the search as `core-document` does; a build by plain `cargo build` shares nothing, and a
build id left unchanged across a change to the search reads stale poses. Reopen when hashing a
model costs a run more than the reads save.

## Alternatives considered

### One render of every pick

Searches once for the pictures — but `set_pose` and every later run that re-stages a pose
search again, and the pipeline after the pose would run once a pick in one process.

### Draw the pictures side by side

Six seconds of thirty-two, each run holding the model.

### The option that won, and what it costs

A file format, a folder the user's cache grows by and a key that must follow the search's
arguments; and hashing a model, some tens of milliseconds on a million faces, every search.

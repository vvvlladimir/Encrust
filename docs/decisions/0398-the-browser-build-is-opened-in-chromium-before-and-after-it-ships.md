# 0398. The browser build is opened in Chromium before it ships and once a week after

- **Status:** Proposed
- **Date:** 2026-11-04

## Context

v0.7.0's browser build went live broken twice over, and nothing noticed. A shader Chrome's
WGSL compiler refuses, though the desktop's accepts it, turned every frame black once a
model was open. The service worker's precache re-fetched the module with `cache: "reload"`
while the page was still streaming it, and on the live host Chromium aborted the page's copy:
every first visit failed. `cargo xtask web --check` only compiles; no test opens a page, and
the maintainer cannot always look at the site after a release. The site deploys from another
repository, so this one can only watch the result.

## Decision

`.github/scripts/web-smoke/smoke.mjs`, on Node and a pinned `playwright`, opens a build in
Chromium's new headless mode — on Linux with SwiftShader as the WebGPU adapter — drops a
20 mm cube on the canvas, and fails on a WGSL or invalid-object warning, a panic, a page
error, a notice on the page, a canvas of one colour, or a drop that changes nothing.

- `release.yml`'s web job runs it on `target/web/dist`, served with the host's headers,
  before the archive is attached.
- `web-smoke.yml` runs it on `encrust.app/app/run/` when a release is published, waiting up
  to an hour for the site to serve it; every Monday; and on demand. The live build passes
  when the version in its `sw.js` is at least the latest release's, so a rebuild of a
  version passes. A failure is retried once five minutes on, then opens one `web-smoke`
  issue or comments on the open one; a pass closes it.

## Consequences

- A refused shader, a panic at start or a blank window blocks the release that carries it.
- A fault of the live host alone, like the precache race, shows within a week, or within the
  hour after a release, as an issue the maintainer is notified of.
- The check costs a Chromium install, about two minutes per run.
- SwiftShader is not a GPU: a fault of one vendor's driver passes. A check that says the
  runner has no WebGPU adapter is the runner, not the build.
- Reopen if the drop stops being the way a model reaches the window, or the hosted runners
  stop offering WebGPU through SwiftShader.

## Alternatives considered

### Validate the shaders with naga in a unit test

Cheap and in Rust, but naga accepted the shader Chrome refused, and it sees neither the
page, the service worker nor the host.

### A Rust browser driver in `xtask`

One language, but a heavier, less common dependency in Cargo for a check CI alone runs, and
it still needs a Chromium on the runner.

### Check the live site every few hours

Faster to notice, but a release is checked within the hour anyway, and between releases the
live build changes only when the host does; the maintainer asked for weekly.

### The option that won, and what it costs

A Node package to keep current beside the Rust workspace, and a check that sees what
SwiftShader draws, not what a user's GPU does.

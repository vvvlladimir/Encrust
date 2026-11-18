# 0399. A reload opens a newer browser build, and the page keeps the module it downloaded

- **Status:** Proposed
- **Date:** 2026-11-04

## Context

Under ADR 0183 a new build waited until every tab of the old one was closed. A reload does
not close the tab, so a user who only reloaded stayed on the old build however often they
tried. That is how v0.7.0's broken first build outlived its fix. On a first visit the page
and the service worker's precache each downloaded the 23 MB module at once, and the page
showed nothing while it came: on a slow link that is minutes of a blank screen.

## Decision

- Before it imports anything, the page checks for a newer build, waiting a bounded time
  for one that is installing, and asks the waiting worker to take over. The worker does so
  only if the asking page is the one tab in its scope. The page then reloads into the new
  build; otherwise it opens the build it has.
- The worker's precache leaves out the module. The page downloads it once, counting the
  bytes into a progress bar, and puts it into the build's cache itself.
- `cargo xtask web` writes the build id and the module's size into the page, as it already
  does into the worker.

## Consequences

One reload opens a new build. A second open tab still keeps every tab on the old build until
it closes, because a running tab's threads import the old script. Users still on a worker
from before this change need to close the tab once more. The page shows its progress on
every start; from the cache it is gone at once. If the page dies before the module is
whole, the next start downloads it again.

## Alternatives considered

### Keep the module in the precache, and fetch it from the HTTP cache

One download as long as the page's copy is already in the HTTP cache when the worker asks.
Nothing promises that order, and `reload` there broke Chromium's own copy (ADR 0398).

### Take over whatever other tabs are open

Simpler, but a tab still running would import the new script over old memory on its
next thread and break mid-job.

### The option that won, and what it costs

The page now carries the update and caching logic the worker had alone, under three time
bounds, and a page and a worker that must agree on the cache's name.

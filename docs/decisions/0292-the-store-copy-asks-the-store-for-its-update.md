# 0292. The Store copy asks the Store for its update and sends the user there

- **Status:** Accepted
- **Date:** 2026-10-16

## Context

ADR 0286 left a copy installed from the Microsoft Store with no word of a new release: it
never reads the feed, and the Store updates it on a schedule of its own, which a user who
wants a fix today cannot see. The copy before 0286's check, 0.3.0, offered its own install
and failed with `Access is denied` replacing a binary under `WindowsApps`. A release reaches
the Store only after certification, hours to days after GitHub has it, so the feed names
versions the Store does not yet hold. The Store answers the question for a packaged desktop
app through `StoreContext.GetAppAndOptionalStorePackageUpdatesAsync`, Windows 10 1607 and
later, with no window handle needed for the look.

## Decision

A window from the Store package asks `StoreContext` once a day, and when asked on the
Updates page, whether the Store holds a newer package for it; the daily switch is not shown,
since the request goes to the Store that updates the copy anyway. A newer package is offered
like a release, at its version (`major.minor.build` of the package, release.yml writing the
release as `major.minor.patch.0`), with no notes and no download. Its button, "Update in the
Microsoft Store", opens `ms-windows-store://pdp/?ProductId=9NB0ZMB3M8H8` through
`Launcher.LaunchUriAsync`, the app's page in the Store app, where the Store's own Update
button installs it. The window still never replaces a Store binary.

`encrust-app` depends on `windows` 0.62, on Windows only, with the `ApplicationModel`,
`Foundation_Collections`, `Services_Store` and `System` features; the lock already held that
version through the graphics stack.

## Consequences

A Store user sees a new version only once the Store can install it, and a click lands on
the button that does. The copy still makes no request to GitHub. The offer carries no
release notes; the release page link beside it has them. A failed look is quiet unless the
user asked, as for the feed.

Reopen if the window should install in place: `RequestDownloadAndInstallStorePackageUpdatesAsync`
does, behind a system dialog, but needs the window's `HWND` through `IInitializeWithWindow`
and closes the app to install.

## Alternatives considered

### The release feed, with a link to the Store

No new dependency. Rejected: it names a release before the Store holds it, sending the user
to a page with nothing to update, and it brings back the GitHub request 0286 removed.

### Installing from the window through the Store

One click fewer. Rejected for now: the dialog needs the native window handle and the app
closes mid-session, neither testable without a Windows machine in the loop.

### The Store's own answer, and what it costs

A Windows API dependency in the app, code that compiles on Windows alone and is checked only
by the Windows build in CI, and an offer without notes.

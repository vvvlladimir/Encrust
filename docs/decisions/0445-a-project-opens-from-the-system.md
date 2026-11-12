# 0445. Register `.encrust` with each system, and take the files macOS hands over

- **Status:** Accepted
- **Date:** 2026-11-13

## Context

A double-click on an `.encrust` file opened nothing. No installer told its system that the
window owns the type. On Windows and Linux the system starts a program with the file as its
argument, and `main.rs` already opens its one argument. macOS never puts the file in the
arguments. It sends an Open Documents Apple Event once the app is running, and AppKit answers
it for an app that declares no handler.

The usual handler is the app delegate's `application:openURLs:`. winit 0.30.13 sets a delegate
of its own, and that delegate starts the window in `applicationDidFinishLaunching:`, so ours
cannot replace it. eframe creates the event loop and runs it in one call.

## Decision

The project file is `application/x-encrust`, a subclass of `application/zip`, with the UTI
`app.encrust.project` and the Windows ProgID `Encrust.Project`. The window is its owner and
editor. The UTI does not conform to `public.zip-archive`, so Finder offers no archive tool to
open it. The table is in `docs/formats/encrust-project.md`.

- **macOS:** `assets/macos/Info.plist` exports the UTI and claims it with `CFBundleDocumentTypes`.
  `system_open.rs` listens for the notification that the app will finish launching, which
  comes before the event that launched it. It then installs its own Open Documents handler
  through `NSAppleEventManager`; Apple's guide puts custom handlers at that point. Paths go
  through `files::arrived` like a browser's files and open by what they are, so a project
  dropped on the Dock icon or passed to `open` opens as a double-clicked one does.
- **Windows:** `cargo packager`'s `file-associations` makes the NSIS installer write the ProgID
  and its open command under `HKCU\Software\Classes`. The Store package declares the same
  extension with `uap3:FileTypeAssociation`, the path passed as `"%1"`.
- **Linux:** the `.deb` installs `assets/linux/encrust.xml` into
  `/usr/share/mime/packages`, and its desktop entry names the type with `Exec=… %F`. The
  shared-mime-info and desktop-file-utils triggers rebuild their databases.

## Consequences

A project opens from a file manager on every installed copy. On macOS the window that is open
takes the file. On Windows and Linux a second window opens, because nothing there hands a file
to a running copy. Linux passes every selected file but `main.rs` opens only the first; the
others are dropped. The `.AppImage` and the archives register nothing, since only an installer
can.

The NSIS template carries three faults from the packager. It writes the program's path
unquoted, though Windows still finds it under a folder with spaces. It does not tell Explorer
the associations changed, so an icon may update only after a sign-in. Its uninstaller
mis-names the field it loops over, so it leaves the association behind. Reopen this when the
packager fixes them, or when one of them costs a user; a template of our own would fix all
three.

A second app declaring the same UTI, such as `cargo xtask app`'s "Encrust Dev", competes with
an installed copy for the default. *Open With* settles it.

## Alternatives considered

### The app delegate's `application:openURLs:`

This is Apple's modern route. It was rejected because winit's delegate starts the window, and
handing it on would mean forwarding every method of a private class.

### Our own event loop through `eframe::create_native`

This would let a handler be installed between creating the event loop and running it. It was
rejected because it needs `winit` as a direct dependency and changes how the window exits.
Listening for the notification needs neither.

### A Windows template or registry writes of our own

These would fix the NSIS faults above. They were rejected for now because they mean keeping a
copy of a 900-line template in step with every packager release.

### The Apple Event handler, and what it costs

It overrides AppKit's own Open Documents handling. A later winit or eframe that handles the
event itself would meet ours, and the one installed last wins.

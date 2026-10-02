# 0104. The title strip is the window's title bar

- **Status:** Accepted
- **Date:** 2026-09-27

## Context

The window carried the system's title bar and, under it, our own strip with the menus and
the settings gear. Two bars of chrome for one window, and the top one
is not ours to paint: on a dark application it reads as a light band that belongs to
something else.

The obvious alternative — move the menus into the platform's own menu bar — was examined
and rejected. It only pays on macOS, where `muda` can hand a `NSMenu` to the application
without winit's help, and even there every command would then live in two places: our own
`egui::MenuBar` and the native one, driven by a separate event channel polled each frame.
On Windows the same crate installs an `HMENU` inside the window, which costs the same
height, cannot be painted from our palette, and is what modern Windows applications are
moving away from. On Linux there is no portable global menu at all: GNOME dropped it, and
the KDE and Unity app menus speak a DBus protocol that wants a GTK backend this
application does not have.

## Decision

The strip becomes the title bar. `main.rs` builds the viewport without the system's one:
on macOS with a full-size content view and the title bar and title hidden, so the traffic
lights keep floating over our strip and it reserves 68 points for them; everywhere else
with no decorations at all, and the strip draws its own minimise, maximise and close at
its right end.

Its height is the platform's: 28 points on macOS, which is the native title bar the system
still lays the lights out in, so our menus and the gear sit on their centre line; 36
elsewhere, where the buttons are ours to draw. Both ends of the strip are laid out in the
strip's own rectangle rather than in the row each would allocate, so nothing on it floats
off that line.

Dragging any part of the strip that is not a control moves the window
(`ViewportCommand::StartDrag`), and double-clicking it maximises or restores. The drag is
interacted with before anything else on the strip, so every control drawn after it takes
the press instead.

## Consequences

- One bar instead of two, and no band above the window that cannot be painted.
- The window is ours to paint from edge to edge. No system band above a dark application.
- Three window buttons to draw and to keep working on Windows and Linux, and a resize
  border the system no longer provides on an undecorated window. winit handles the resize;
  if a platform turns out not to, that is a bug to chase rather than a decision to revisit.
- macOS reserves a fixed 68 points and a fixed 28 of height. A user who scales the system
  UI could crowd the menus; both numbers would then have to be read from the platform
  rather than written down.
- Anything that assumed a native title bar — a window snap gesture, a screen reader
  announcing the window title — now sees a plain undecorated window.

## Alternatives considered

### Keep the system title bar

Free, and every platform behaves the way its users expect. Rejected because it is a second
bar of chrome that cannot be painted, above an application whose whole point is one dark
surface.

### Move the menus into the platform menu bar

Examined above. Rejected: it pays on one platform of three, and doubles where every command
lives.

### The option that won, and what it costs

An undecorated window is a window whose manners we now own. Drag, double-click, maximise
and the buttons are ours to get right on three platforms, and each of them has a habit we
are breaking. The macOS inset is a magic number until someone asks the platform for it.

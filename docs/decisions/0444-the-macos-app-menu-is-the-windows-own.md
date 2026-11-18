# 0444. Build the macOS application menu in the window, and date About by the binary

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

winit puts up a default application menu named after the running process, so macOS showed
*About encrust-gui*, *Hide encrust-gui* and *Quit encrust-gui*. The binary cannot simply be
renamed `Encrust`: `encrust`, the command line, stands beside it in `Contents/MacOS`, and the
default file system ignores case.

The standard About panel reads its version from the bundle's `Info.plist`. The window's own
update replaces the binaries and nothing else (ADR 0172), so after an update the panel named
the version first installed: 0.2.0 over a 0.8.0 build.

A menu row that reaches the window, Settings or an update check, needs an Objective-C object
as its target, and the menu needs `NSMenu` and `NSMenuItem`.

## Decision

On macOS `app_menu.rs` replaces winit's menu on the first frame and again when the language
changes. The rows are About Encrust · Settings…, Check for Updates… · Services · Hide
Encrust, Hide Others, Show All · Quit Encrust, in the window's language and with the words
macOS gives them.

- About opens the standard panel with the version taken from the binary and no build number.
  The copyright comes from `NSHumanReadableCopyright` in `assets/macos/Info.plist`.
- Settings… opens the Settings window. Check for Updates… opens it on Updates and looks now.
  An object of a class declared with `objc2`'s `define_class!` is the rows' target, and it
  hands what was asked to the next frame.
- Settings… carries no key in the menu. ⌘, stays the keymap's, so a user who binds Settings
  to another key keeps it.

`objc2` 0.6 becomes a direct dependency on macOS. It is already in the tree through winit, so
nothing new is built.

## Consequences

The menu speaks the product's name and the window's language, and About tells the truth after
an update. The menu is ours to keep in step with macOS: a row AppKit adds by convention
(Window, Help) is not there until it is built here. Windows and Linux have no application
menu, and About stays macOS-only until the window gets a panel of its own.

Reopen this if the window moves File, Edit and View into the system menu bar. A menu library
then pays for itself.

## Alternatives considered

### Rename the main binary to `Encrust`

This would fix the name with no code. It was rejected because it clashes with `encrust` on a
case-insensitive volume, and it leaves About's version wrong.

### A cross-platform menu crate

It would bring a ready menu and room for File, Edit and View later. It was rejected because
it adds a dependency tree for eight rows that AppKit already provides.

### `objc2` directly, and what it costs

Some `unsafe` code holds AppKit's contracts: a menu item holds its target weakly, so the
target lives for the whole run, and a row's action must take the sender alone. These are
checked by hand, not by the compiler.

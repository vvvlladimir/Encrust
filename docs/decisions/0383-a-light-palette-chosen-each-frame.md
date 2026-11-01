# 0383. Add a light palette, chosen each frame from egui's resolved theme

- **Status:** Accepted
- **Date:** 2026-10-30

## Context

The window had one palette: `theme::colors()` and `theme::scene()` were `const fn`s returning
one `&'static` set each, read from over a hundred files. Users asked for a light window, and
for it to follow the system's light or dark setting as that changes, on every platform the
window runs on. egui already resolves a theme each frame: `ThemePreference::System` reads the
system theme eframe reports (winit on macOS, Windows and Linux where the desktop says; the
`prefers-color-scheme` query in a browser), with dark as the fallback.

## Decision

`ui::theme` holds two palettes and two scenes, `DARK` and `LIGHT` (slate: cool blue-grey
surfaces under a white panel, the ember accent unchanged, a burnt orange where the accent is
text). `theme::apply` installs a style for each of egui's two themes. The user's choice,
`Appearance` (System, Light, Dark) in `General`, is handed to egui as its `ThemePreference`;
once a frame, before drawing, `theme::follow` stores whether egui resolved light, and
`colors()` and `scene()` return the matching set. Every call site stays as it was.

## Consequences

Switching is instant and follows the system without a restart. Shadows and the scrim are
palette tokens now (`shadow`, `scrim`, and `knob` for a dragged handle), since black at the dark theme's alpha is a smear on
slate. The light ladder runs the other way above the panel: raised, hover and active darken.
Each palette is held to AA by the same tests. Whatever caches a colour across frames — a
texture, a pipeline constant — would keep the old theme; none does today, and a new one must
read the palette per frame. Reopen if a second window needs a theme of its own.

## Alternatives considered

### The palette passed through `egui::Context` to every panel

Explicit, and a second window could differ, but every one of the hundred files reading
`colors()` changes for no difference a user sees.

### The option that won, and what it costs

A process-wide flag the frame sets: a test that flips it would race the snapshot tests in the
same binary, so no test flips it, and the light palette is checked as data instead.

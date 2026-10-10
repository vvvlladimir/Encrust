# 0216. The window is set in IBM Plex, on graphite, with figures in the text face

- **Status:** Accepted
- **Date:** 2026-10-10

## Context

[ADR 0024](0024-design-tokens-and-bundled-typeface.md) gave the window one token module and
compiled Geist and Geist Mono into it, with every figure set in the mono face because Geist's
default digits are proportional and egui cannot ask a face for its `tnum` feature: epaint 0.36
lays text out with no OpenType features at all.

The window is being redrawn after a mockup whose surfaces are a cooler graphite ladder, whose
accent is a brighter ember with a soft tint for text on dark, and whose figures sit in the text
face with tabular digits, keeping mono for what is read as code. A face used for a technical
look is one of the patterns the redraw sets out to drop, and two faces in one row of fields
make that row read as two things.

IBM Plex Sans ships its default digits at one advance, 600 units in every weight we compile in
(checked with fontTools on the release's TTFs), so its figures line up in a column without the
feature egui lacks.

## Decision

`assets/fonts/` carries IBM Plex Sans Regular, Medium and SemiBold and IBM Plex Mono Regular,
from IBM's own release of each, with their SIL OFL 1.1 terms in `IBMPlex-OFL.txt`. Geist is
removed. `theme::figures` sets millimetres, seconds, counts and sizes in Plex Sans;
`theme::code` sets an address, a file name or a report in Plex Mono, and egui's `Monospace`
style is that face.

The palette becomes the mockup's: surfaces `sunken #0c0e11` to `hover #272d34` and a new
`active #313840` above them, `hairline #252a31`, `line #39414b`, ink `#efebe5` / `#a9a7a3` /
`#8a8e94`, accent `#f2763a` with `accent_deep #c9531f` and a new `accent_soft #ffb184` for the
accent as text or glyph. A selection washes in the accent rather than in green. The model is a
cool resin `#7fa6b6`, supports a warm grey `#c9c2b6`, and a cut face `accent_soft`.

Density follows: field 28, row 30, item gap 8, a named field's box 128 wide with its unit
inside it, button 32, primary 40, section heading 36, columns 272 / 328 / 68, and a window
rounded by 10 beside controls by 6 and cards by 8.

`cargo xtask licenses` appends the typefaces' terms to `THIRD-PARTY-LICENSES.md`, since
cargo-about only sees crates and the faces were shipped without theirs.

## Consequences

A row of fields is one face, and a number reads as part of the sentence it is in. The binary
grows by about 250 kB over Geist. The luminance and contrast tests hold the new values as they
held the old: `active` joins the ladder, and every ink still meets AA on `panel`.

Figures line up only as long as the face keeps one advance for its digits: a face swapped in
later has to be checked the same way, or numbers go back to mono. The `Plex` name is reserved
by IBM under the OFL, so a modified copy of the face could not keep it; we ship it unmodified.

## Alternatives considered

### Keeping Geist, with Geist Mono for figures

No new asset. Rejected: it is the mono-for-figures look the redraw drops, and Geist's
proportional digits leave no way back to the text face while egui has no `tnum`.

### Plex Sans for text, Plex Mono for figures

Safe if the digits had turned out proportional. Rejected once they were measured: it keeps two
faces in every field row for no gain.

### Plex for everything, and what it costs

A face with a reserved name, about 800 kB of TTF in the binary, and a dependence on its digits
staying tabular that nothing but a person checking a release enforces.

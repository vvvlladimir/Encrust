---
paths:
  - "crates/encrust-app/**"
  - "crates/encrust-web/**"
  - "assets/i18n/**"
---

# UI rules

Read `docs/design/ui-standards.md` (how) and `docs/design/ui-design-system.md` (what exists)
before changing a panel.

- Panels compose `ui::widgets` only: no stock egui control, no painting, no styled label, no
  token outside `ui::theme`. A missing widget is added there, with its catalogue row.
- A tool panel is heading (modes as icons), preset, sections, foot — in that order.
- Every string is `tr!("key", ...)`, English in `assets/i18n/en.ftl`, one sentence a
  message, never assembled from pieces (`docs/design/i18n.md`).
- Explanation is a `describe` behind the question mark; printed text is only why something
  cannot be used, a verdict, or a one-line empty state.
- Units and decimals come from the standard's table, the same everywhere.
- A new value lives in `ToolSettings`, `Preferences` or `ui::kept`, so it is remembered and
  undone; the irreversible asks first. Nothing longer than a frame on the drawing thread.

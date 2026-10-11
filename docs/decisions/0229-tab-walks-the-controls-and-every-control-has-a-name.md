# 0229. Tab walks the controls, and every control has a name

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

ADR 0193 took every key the table binds out of the raw input before egui read it, Tab
among them: Tab switched between the model and the layer, so egui's ring of focusable
widgets was never walked and the window could not be driven without a pointer. 0193 named
the signal to reopen it: the window being asked for keyboard-only operation. The v2 window
is asked for exactly that, and for a screen reader to find its controls.

Most controls are painted by hand from `theme` tokens, not built from egui's widgets, so
none of them told AccessKit what they were, and none showed when it held the focus.

## Decision

- **Tab is egui's.** Tab and Shift Tab walk the ring of controls; the three views of the
  layer strip are stepped through with M, and back with Shift M. A click does not focus a
  button in egui, so the pointer never leaves one holding the keyboard.
- **The focused control keeps the keys that drive it.** While a control that is not a
  field holds the focus, `shortcuts::take` leaves Enter, Space, Esc and the bare arrows to
  egui: Enter and Space press the control, Esc lets go of it, the arrows move the focus.
  Every other key, Shift and the arrows included, is still the table's. The layer track,
  once focused, keeps the bare arrows for itself and steps the layers with them. A field
  being typed into still owns the keyboard whole.
- **Every control has a name.** Each hand-painted control states its role, its name and,
  where it has one, whether it is on, through `Response::widget_info`; a number box, whose
  name is painted beside it, carries the name on its AccessKit node.
- **The focus is drawn.** A control holding the focus wears a ring in the accent inside
  its own edge, `theme::FOCUS_W` wide (`ui::focus_ring`).
- A surface steered by the pointer alone — a resize grip, the title bar's drag zone, the
  mask's picture, the view cube — is not focusable, so Tab never stops on it.

## Consequences

The rail, the inspector, the top bar and the layer strip can be walked and pressed from
the keyboard and read by a screen reader. A test focuses each tool of the rail by its
AccessKit name and presses Enter on it.

Tab no longer switches the view; the sheet and the strip's tooltips say M. Esc on a focused
control lets go of it rather than deselecting, so it takes a second Esc. The view cube and
a double click — editing a resin, renaming a plate — have no keyboard way in; the Frame
view key and the menus stand in for the first.

A new hand-painted control needs both calls, `widget_info` and `focus_ring`, or it is
invisible to the ring and silent to a reader. Reopen this if that is forgotten often
enough to want one helper that allocates, names and rings a control in one call.

## Alternatives considered

### Keep Tab on the view, and walk the controls with another key

No key to relearn. Rejected because Tab is the key every platform and every screen reader
user expects to move between controls, and egui walks its ring on it alone.

### Hand egui every key while a control holds the focus

Simpler than a list of the keys a control uses. Rejected because it is 0193's original
bug: one Tab would leave the whole table dead until the focus was dropped.

### The decision above, and what it costs

The meaning of four keys now depends on whether something holds the focus, which a user
cannot always see across the window, and two calls have to be remembered on every control
painted by hand.

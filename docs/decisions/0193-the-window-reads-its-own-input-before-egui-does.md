# 0193. The window reads its own input before egui does

- **Status:** Accepted
- **Date:** 2026-10-06

## Context

Every key the window answers lives in one table (ADR 0106), read inside the frame with
`InputState::consume_shortcut` and skipped whenever `egui_wants_keyboard_input()` was
true. That call is "any widget has the focus", and egui gives the focus to the first
focusable widget it draws as soon as Tab is pressed — it reads Tab, and the four arrows,
out of the raw input before a frame begins. So one Tab switched the mode and handed the
focus to a button, and from that frame on every key in the table was dead: Tab included.
The transport keys were worse off still, gated on the Preview mode although the section
rail is one rail in both (ADR 0061).

The pointer has the same shape of problem from the other side. `transform-gizmo-egui`
registers an interaction widget under the cursor on every frame it draws, so the viewport
cannot use its own `Response` and reads the raw pointer instead (`viewport_input.rs`). It
therefore had to be told what was drawn over it: `panels::Overlays` carried the rectangle
of every card from the previous frame. A popup or a floating window was not in that list
unless somebody remembered to put it there, so dragging a field in the Array popup orbited
the plate behind it.

And a tool acted on whatever model the click landed on. With one part selected and the
Drain tool in hand, the first click on a second part drilled it rather than aiming at it.

## Decision

The window reads its own input first, in one place each.

- **Keys.** `shortcuts::take` runs in `eframe::App::raw_input_hook`, before egui's pass: it
  takes every chord the table binds out of `RawInput::events` and answers with the
  actions. egui never sees those keys, so it cannot walk its focus ring with them or leave
  a widget holding the keyboard. A text edit with the focus gives the keyboard away whole —
  nothing in the table fires, and Tab moves between fields as egui intends.
- **The transport belongs to the rail.** `section::step` and `section::play` answer in both
  modes: the stack in Preview, the cut through the models in Prepare, which runs up the
  model at the rail's own rate and stops at the top with the whole model shown again.
- **The pointer belongs to the topmost layer.** The viewport is under the pointer only when
  `Context::layer_id_at` names its own layer there, which is true of every card, popup,
  menu and floating window without any of them reporting a rectangle. `Overlays` is gone.
  A popup that carries fields rather than actions closes on a click outside it, not on any
  click, so a click inside opens the field it landed on for typing.
- **A click outside the selection aims rather than acts.** While something is picked, a
  click on a model that is not picked only picks it; the click after that is the one the
  tool acts on. With nothing picked a tool still has the whole plate, so the click goes
  straight through (ADR 0101). A drag is not a click: the brush paints what it is dragged
  over, and the Edit mode already picks a part before it carries one.

## Consequences

One place says what a key does, one rule says who owns the pointer, and a card added
tomorrow is covered by both without being registered anywhere.

Tab no longer reaches egui's focus ring outside a text edit, so the window cannot be
driven from the keyboard alone beyond the table: the rail, the buttons and the pickers
want the pointer. Reopen this if the window is ever asked for keyboard-only operation; the
table would have to grow a way to move the focus rather than egui's being handed Tab back.

Aiming a tool at another part now takes two clicks instead of one. That is the trade the
report asked for: a hole drilled where the user meant to look costs an undo, and a hole
they never asked for cannot be seen until the print comes out of the vat.

## Alternatives considered

### Keep reading the keys inside the frame, and clear egui's focus after each shortcut

The smaller change, and it would have fixed BUG-18's symptom. Rejected because egui has
already decided to move the focus by the time any of our code runs — `Focus::begin_pass`
sets the direction from the raw events — so the ring would still be walked on the frame
the shortcut fired, and the fix would depend on a private ordering inside egui.

### Register every card, popup and window in `Overlays`

What the code did. Rejected because it is a list to keep in step by hand: every popup
added since has been a bug waiting for somebody to drag a field in it, and egui already
knows the answer.

### Let a tool act on whatever the click landed on

One click instead of two, and the selection is only a default scope. Rejected by the
report: it is the behaviour that drilled a hole in the wrong part.

### The decision above, and what it costs

The window now takes keys away from the toolkit it is built on, which is a thing to
remember when egui's own shortcut handling is wanted for something later, and the
viewport's idea of "under the pointer" is read out of egui's layer order rather than from
anything this code states. Both are fewer moving parts than what they replace, and both
are invisible until they are wrong.

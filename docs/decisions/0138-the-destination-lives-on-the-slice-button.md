# 0138. The destination lives on the Slice button

- **Status:** Superseded by 0157
- **Date:** 2026-09-29

## Context

A stack can now go to a file or to a printer on the network. One answer is a separate
*Network Sending* button, which opens a window asking for a file name, a format and a
printer. The FDM slicers instead turn the export button into *Send G-code* when
the selected printer has an address.

The inspector footer already holds one primary action with the container it writes hanging
off a caret beside it, because the format is a property of that action and of nothing
else. Everything that guards the action — why it is greyed out, what the stack will come
to, the progress bar, the choice of this plate or every plate — is attached to it.

## Decision

The destination is another property of the same button. The caret beside *Slice* lists
the formats, then a separator, then `File...` and every printer a scan has found; the
button names where it is going. A printer chosen there makes the button write into a
temporary file and send it, and the file is deleted once the transfer ends either way.

Starting the print is a second, deliberate press, on a button that appears once the file
has landed. Uploading does not start a print.

## Consequences

There is one entry into slicing, so the blocker, the estimate and the progress bar are
written once. A file the user names stays where they put it; one the window wrote for
itself does not outlive the transfer. `Slice all plates` disappears while a printer is
the destination: a queue of transfers is not what that button means.

The second press is the point of the decision. A resin print starts by dropping a plate
into a vat; a click made three minutes earlier, on the other side of a transfer whose
length nobody predicted, is not consent to begin. The cost is a press the user cannot
avoid even when they are standing at the machine.

The printer list carries no live state — a name, a model and an address, as discovery
reported them. Showing whether a machine is busy means holding a control socket open to
every one of them, which is a background connection per printer for a line of text.
Reopen that when the window has somewhere to show a machine continuously.

## Alternatives considered

### A Network Sending button beside Slice, opening its own window

The separate button, and it makes the feature obvious. It lost because the window would
have to ask again for everything the footer already knows, and a second entry into slicing
means the blocker, the progress bar and the plate choice exist twice.

### Starting the print automatically once the file lands

One press instead of two, which is what a user standing at the printer wants. It lost on
the vat: the machine may be empty, dirty or still holding the last print, and nothing in
the window can see that.

### The option that won, and what it costs

A destination inside a caret menu is discoverable only by opening it. A user who has never
pressed that caret does not know the window can reach their printer at all.

# 0348. Slice and send from the plate's card, and stand the rail over the inspector

- **Status:** Accepted
- **Date:** 2026-10-25

## Context

The Export tool repeated the plate's card. Its file section stated the height, the layers,
the resin, the price and the time the card under the models already stated, and its foot
held the same Slice the card holds. What it had of its own was the file's name, the button
to the print settings the top bar already opens, the plate scope the caret beside Slice
already offers, and the printer the file is sent to. A sent file waiting to be started
had its notice on the stage, with Start, apart from the press that sent it.

The rail was a column the stage's height beside the inspector, spreading eight tools down
a tall screen so that a short list read as a long one, and taking 68 points of width from
the stage for them.

## Decision

- Export leaves the rail and `workspace::Tool`; Finish is Check alone.
- The plate's card is the way out of the window: its figures, with the print time once a
  stack cut from the plate as it stands is planned; the printer row of the profile's
  listed machines when the profile sends over a network; then one press.
- Slice to a file is always there. Over it stands Slice and send to the bound machine once
  it has answered and can take the file — it speaks the profile's protocol, is not busy;
  otherwise there is no such button and no line saying why, the printer row above already
  stating the machine's state, with a scan again. Pressed, it greys, and the bar of the
  slice, then of the upload, stands over it; once the file landed it is Start print, still a
  second deliberate press, and the stage's notice of a sent file goes. The top button is the
  accent one, Slice to a file raised under it. Every plate at once is an item of the File
  menu, Slice to a file another on Cmd Enter. The file is named in the
  save dialog, or after the model when it is sent; the window keeps no name of its own.
- The rail is one row of tools over the inspector, as wide as it and sized by the same
  drag, a rule between each group. Until dragged, the column stands as wide as the rail
  with every tool's name under its glyph, each button as wide as its name; a double click
  on the edge returns it there. A window too narrow for that squeezes the column, and the
  rail falls to its glyphs in squares; the names come back as it widens. With the inspector
  put away the column is the rail's glyphs alone; with Preview's mask alone it is gone.

This supersedes ADR 0218's Export tool, its dot and its column of tools, and ADR 0157's
caret beside Slice.

## Consequences

- The figures of a plate are stated once, where the press that uses them is.
- Sending is one press when a printer answers, starting it a second at the same place; one
  not yet asked, busy or silent leaves only the file, rather than a greyed button or
  sending blind. Slice to a file never moves its meaning under the pointer.
- The stage gains the rail's width, and a tool is picked right over the card it opens.
- The inspector is as wide as the rail's names, wider than the 328 it was, by language;
  a width dragged before is forgotten once, kept under a new key.
- The volume, weight and price measured off the stack are read in Preview, where the card
  states the stack's figures; the model view states the estimate from the meshes.
- A printer profile taking files on a stick shows no row and no hint; the reopen signal
  is a user who cannot find where to send from.

## Alternatives considered

### Keep Export and drop the figures from the plate's card

The tool would own the file, and the card would be Slice alone. Rejected: the figures
belong with the press in every tool, and the tool was opened only to read them.

### One press that turns into Slice and send while a printer answers

What the card had first in this record. Rejected: the press changed its label as a printer
came and went, and a line under it had to say why it wrote a file.

### A caret beside the press for the other destination and every plate

What the card had first. Rejected: a second button for what is rarely wanted, in the one
place the eye goes to start a print; the menu bar holds it with its key.

### A rail across the top of the stage

The whole width would name every tool. Rejected: the tools then stand far from the card
they open, and the top bar already holds the machine and the settings.

### Equal buttons whose names show once the widest fits

What the rail had first. Rejected: at the default width no name ever showed, in Russian at
no width at all, and a drag across the threshold made the rail jump 12 points in height.

### The option that won, and what it costs

Every plate at once is a menu away. A second button stands on the card while a printer
answers, pushing Slice to a file down by a row, and the tools' names take a wider inspector than its content needs.

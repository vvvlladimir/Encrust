# 0200. A pocket of trapped resin is painted on its own

- **Status:** Accepted
- **Date:** 2026-10-07

## Context

ADR 0190 paints the cavity of a model the drainage check found resin in, out of the shell's
own cavity faces, and says plainly what that costs: it is the whole cavity, not the pocket,
so a cavity drained everywhere but one corner still lights whole. It names the reopen signal
— a user reading that as "none of this drains" — and the fix: the pocket's own box out of
`TrapScan`, tested against in the shader.

That is what happened. A bracket of three boxes hollows into three pockets, the panel
counted three, and the x-ray lit all three the same. Drilling a hole into one changed
nothing on screen, so three real pockets read as one imaginary one that could only be
fixed all at once.

The cavity is one mesh; nothing in the face range says which part of it belongs to which
pocket. The scan does know: it walks the air of each layer and joins the runs that touch,
so the cells a pocket reached are in hand while it is alive.

## Decision

`Trapped` carries `bounds`: the box the pocket fills, plate millimetres, out to the edges
of the cells it reached and half a layer past the heights it was seen at. The window brings
it into the model's own space with the point where a hole goes, and back onto the plate as
the box of its eight corners when the frame is drawn.

The globals carry up to `MAX_POCKETS` = 32 of those boxes, and the model fragment shader
discards a **volume** fragment — the cavity's red, never a surface — standing in none of
them. Past 32 a pocket goes unpainted; the panel still counts it.

## Consequences

Three pockets read as three. Draining one and checking again takes its own red off and
leaves the others, which is the answer the Drain tool's work has to be judged by.

A box is not the pocket: an L-shaped pocket lights the corner it does not fill, and two
pockets whose boxes overlap each light a little of the other's space. That is a smaller
over-statement than the whole cavity and it costs no geometry. The signal to reopen is a
user drilling into a corner the box lit and the pocket never reached; the fix after this
one is the pocket's own runs, which means keeping what the scan streams away (ADR 0190).

A model turned on the plate has its box turned with it and grown to stay axis-aligned, so
the red spreads a little on a rotated part.

## Alternatives considered

### Mesh each pocket out of the scan

Exact, and the scan has the runs while the pocket is alive. Rejected again on ADR 0190's
own ground: it holds a cavity's worth of runs instead of streaming them, and builds and
uploads a second mesh the size of the cavity for a picture.

### Cut the cavity's face range per pocket

Keep the no-geometry trick and pick the faces of the shell that bound one pocket. There is
no such range: the extraction lays the cavity out in lattice order, not pocket by pocket,
and finding the faces would be a connected-component pass over the cavity on every check.

### The option that won, and what it costs

A box over-states a pocket that is not box-shaped, and a bound of 32 that a cavity full of
sealed infill cells could pass. Both fail towards painting too much or too little of a
picture, never towards a wrong report: the count and the volumes come from the scan.

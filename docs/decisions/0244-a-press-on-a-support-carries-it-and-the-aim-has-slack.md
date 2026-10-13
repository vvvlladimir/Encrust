# 0244. Carry a support by the press that picks it, and give the aim slack

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

ADR 0095 made editing two presses: the first picked a part, the second carried it. The
first press held the pointer only for its own frame, so the drag that followed it turned
the camera — the commonest gesture in the mode, press and pull, spun the view and moved
nothing. `grab` took a part only when the ray went through it, and a pillar is about a
millimetre across: from a camera framing a whole plate that is a few points of screen.
Nothing showed which part a press would take until it had been taken.

## Decision

A press on a part picks it and the same drag carries it. A part not yet picked replaces
what was; one already picked keeps the others, so several are carried by any one of them.
Shift only adds a part to what is picked or takes it out, and that drag carries nothing.
From a press that lands on a support to the release the gesture is the support's, wherever
the cursor goes: the camera neither turns nor slides.

`grab` takes a `slack_mm`. A part the ray goes through wins as before, nearest first; when
there is none, the part the ray passes nearest within the slack is taken. The viewport
sets the slack to two points of screen, measured where the camera is looking: enough to
hit a pillar a point wide, too little to take a part the cursor is not on. The part a
press would take is drawn in the picked colour, fainter, while no button is down.

A tip held alone is aimed rather than stepped: it goes where the cursor's ray meets its
model, else to the surface nearest the cursor at the tip's depth, and its neck goes with
it. A step on a plane facing the camera lifts a tip off the surface it must touch, and
with the neck left behind the head ran through the model and every such step was refused.

## Consequences

Moving a support is one gesture, and a thin pillar can be caught from a whole-plate view.
The drag is measured from where the cursor was pressed, so a stick taken by its end does
not jump to put its middle under the cursor, and a step the model refuses whole is retried
as its run across the plate and then its rise, so a part slides along what it touches. A
press that only meant to pick now also moves the part if the hand shakes; undo covers it. The slack is measured at the camera's target, so in perspective a support far behind
it is caught from a little further than two points and one in front from a little nearer.
Reopen this if picking without moving turns out to need a dead zone of its own.

## Alternatives considered

### Keeping two presses and only holding the pointer after the first

It stops the camera turning, but press-and-pull still does nothing the first time, which
reads as a tool that is not listening.

### Widening every part by a fixed number of millimetres

A millimetre is nothing from far away and swallows the neighbours up close; the aim is
made on the screen, so the slack is measured there.

### The option that won, and what it costs

A press cannot pick a part without also being ready to move it, and `grab` has one more
argument every caller has to think about.

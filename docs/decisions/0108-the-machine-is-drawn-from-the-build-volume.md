# 0108. The build platform is drawn from the build volume, not loaded from a model

- **Status:** Accepted
- **Date:** 2026-09-27

## Context

The viewport drew the plate as a grid on nothing: a rectangle of lines in empty space,
with the build volume as a wireframe above it. Nothing said which way the machine faces or
what the models were standing on, so a model that had been auto-oriented looked the same
whichever edge was the front. Every other slicer answers this with the platform under the
plate, some of them with a per-printer mesh and a texture named in the printer profile.

A profile-supplied bed model is real work: a new asset kind in `printer-profiles`, a place
for it in the build script's embedding, a fallback for every profile that has none, and a
mesh per printer to draw and keep.

## Decision

The platform is generated from the numbers the `Plate` already carries: a deck of a fixed
thickness standing a fixed lip past the plate, its underside drafted in so the top reads
apart from the sides, and the arm it hangs from at the back with a levelling knob on it.
`render/machine.rs` builds it as a triangle list of `BodyVertex`.

An MSLA machine cures downwards, so the platform's structure is on the far side from the
print: the arm hangs under the deck rather than standing past it, and nothing the machine
draws is above `z = 0`. The vat is not drawn at all — it would be a tub around the print
on the side the print is on, which is exactly the space the viewport is for.

The deck's top is a face over the whole plate rather than a frame around it, so the plate
reads as a surface the models stand on. It is drawn after the grid with the depth test on
`Less`, so it loses every tie and the lines stay crisp on their own pixels. The machine is
drawn last, translucent, with depth writes off, the arm before the deck so the deck reads
as being in front of it.

It has a lit pass of its own — `body_vertex`/`body_fragment` — rather than sharing the
model pipeline, whose shader is the print's: it cuts at the section plane, subtracts drain
holes and washes exposure bands, none of which a printer wants.

`Front` is painted on the lip: the font lays the word out, and the triangles egui already
tessellated it into are mapped from points onto the lip in plate millimetres and drawn in
the 3D pass, sampling egui's own font atlas. It is a marking on the machine, so it takes
the machine's perspective and does not move when the camera does. The word never changes,
so it is laid out once and the atlas behind it snapshotted once — safe because egui only
appends to that atlas, so the glyphs keep the coordinates the layout gave them.

## Consequences

The platform costs about two hundred vertices a frame and no mesh, no asset and no loading
path. It cannot be wrong for a printer, because it claims nothing a printer profile does
not already state. It is also generic: an Elegoo and an Anycubic get the same deck, and
nothing distinguishes one machine from another but its build volume.

The word costs a second bind group in the viewport — a texture and a sampler — which is
the first thing in this renderer to have one, and a whole copy of the font atlas on the
card beside egui's own. That is one upload of a few hundred kilobytes for the life of the
window.

Translucency without sorting means the blend inside the platform is order-dependent in
principle. It is not in practice, because every face is the same colour and alpha, which
makes `over` commutative between them; the one order that matters, arm before deck, is
fixed in the buffer.

The signal to reopen this is a printer whose shape a user would recognise and miss.

## Alternatives considered

### A bed model and texture in the printer profile

The honest answer, and where this goes if it goes anywhere. It lost on cost: it is an
asset pipeline and a mesh per printer, for a step that is about seeing the plate, not
about modelling the machine.

### The word cut as geometry from a stroke font

Tried first: five glyphs as polylines, each segment a flat bar. It needs no texture and no
second bind group. It lost on how it looked — a hand-cut stroke font has uneven joints and
none of the fitting a real face has, and it read as worse than the font the rest of the
window is set in.

### The word as an egui overlay turned to follow the lip

Also tried: laid out by the font, drawn as chrome, rotated to the lip's direction on
screen. Crisp, and nearly free. It lost because it is not on the machine: it keeps its own
size and stays flat while everything under it turns, so it reads as a label floating over
the plate rather than as a word printed on it.

### The option that won, and what it costs

A fixed-size deck and arm are a lie about every machine they are drawn for. They read as
"a printer", not as "your printer", and a user comparing the proportions to their Saturn
will find them wrong. That is the price of drawing something rather than nothing, and it
is paid in a part of the picture nobody measures against.

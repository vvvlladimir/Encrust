# 0151. Let the preview show a source, which is either a plate or a file

- **Status:** Accepted; what ends a file's turn is amended by 0202
- **Date:** 2026-09-30

## Context

Step 18d2 puts an opened sliced file in the window. The mask pane already does everything
such a file needs shown: a layer under a slider, zoom, pan, a magnifier, a full-resolution
detail tile, and a texture cache keyed on what the picture was drawn from (ADR 0023).

What it did not have was a second place for layers to come from. `Preview` held a
`Stack { mesh, windows, cut }` and reached into it from six methods, because a plate is
previewed by **cutting the layer being asked for** rather than by holding a stack
(ADR 0068). An opened file answers the same question — the runs of layer `n` — by decoding
it, and holds no stack either.

Two things about a file are not true of a plate. Its panel is its own: a printer profile
loaded in the window says nothing about a file another slicer wrote, and drawing a
1620 x 2560 file on an 8520 x 4320 profile's panel would put the mask in the wrong place.
And it has no contours, so the area a layer exposes cannot be taken from them.

## Decision

`Preview` holds a `Source`, which is a plate being cut or a file being read. Both answer
`runs_of(index)`, and the slider, the transport, the texture cache, the downsampling and the
detail tile are untouched by which one it is.

**The panel comes from the source.** `Preview::read_panel` builds `RasterSettings` from what
the container states, and the mask pane prefers it over the profile's. A container that
records no panel size — a `.ctb` states its bed, not its display — yields `None`, and the
section says so rather than inventing a pitch.

**A layer's area comes from the mask when there are no contours.** The runs held for an
opened file are the panel's own resolution, not the shrunk picture, so counting lit pixels is
exact; ADR 0023's warning about inflated area applies to the shrunk mask, which this is not.

**A file is never stale and is never measured.** Nothing about the scene makes it out of
date, and what it cures is not ours to work out from a mesh. The window shows what the
container states.

**The file is opened, not imported.** `core_pipeline::reads_sliced_file` decides from the
name alone, so a drop, the dialog and a path on the command line all route the same way: a
mesh to the plate, a sliced file to the slider.

## Consequences

Every container this project writes can be looked at layer by layer, at the resolution its
own writer used, including files no part of this project produced. The step-18 milestone —
a sliced file from another slicer opens here — is met.

The picture of an opened file is the picture of a written one, because it is the same code:
a bug in the mask pane shows up in both, and so does a fix.

`Preview` now has methods that answer for one source and not the other — `plan` is `None`
for a file, `measure` returns without doing anything. That is honest about what each source
can say, and it is why they are `Option` rather than a value invented to fill the gap.

What the window does **not** show is what an opened file's masks cure over the whole stack,
which `encrust-cli --read` does print. Getting it would mean decoding every layer, and the
reader is not `Send` (ADR 0150), so it cannot go on the thread that measures a plate without
a bound nothing has asked for yet. The per-layer area is shown instead. The signal to revisit
is a user who wants to check a foreign file's resin against its own header.

## Alternatives considered

### A panel of its own for opened files

The first shape considered. It loses the zoom, the pan, the magnifier, the detail tile and
the texture cache, or copies them — and two mask viewers would drift apart the first time one
was fixed.

### Rasterise an opened file's runs through `ScanlineRasterizer` like a plate's

Keeps one path into `Shown`. It cannot work: a rasteriser turns contours into runs, and a
container holds the runs already. There are no contours to give it.

### Draw an opened file on the loaded profile's panel

What falls out of changing nothing. It puts a file's mask at the wrong scale and in the wrong
place whenever the profile is not the machine the file was written for, which is most of the
time — the reason to open a foreign file is that it is foreign.

### The option that won, and what it costs

`Source` is an enum, so every method that reaches into it branches, and a seventh reader of
the same shape would make that a trait instead. It also means the window's Preview mode has
two meanings, and a user who has a file open has to close it to get the plate back — there is
a menu item for that, and nothing else says which one is on screen.

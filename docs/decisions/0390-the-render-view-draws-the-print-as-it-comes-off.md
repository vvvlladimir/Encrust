# 0390. Draw the print as it comes off the printer in a view of its own

- **Status:** Accepted
- **Date:** 2026-10-31

## Context

The working view is flat-shaded in theme colours so marks read on it (ADR 0219). A user
also asks what the part will look like and how big it is: in its resin, with its layers,
clear or not, beside something of known size. AR answers the size question only on a phone
and needs a mesh writer the workspace lacks. The window runs on WebGPU and on a WebGL
fallback with a slow-frame budget (ADR 0259), and the scene is already drawn into planes of
its own and copied out (ADR 0184), with no multisampling.

## Decision

**A view toggle, *Show as printed*, beside the x-ray in the view tools and the View menu.**
It draws every model and support in the resin's colour under a procedural studio light,
steps the layer height into the surface in the shader, and lays a ruler, a bank card, a
one-euro coin and a 32 mm figure on a 25 mm base off the plate's right edge. Picking,
slicing and the written file do not change.

**A resin carries `details.clarity`, 0.0 opaque to 1.0 water-clear**, default 0.0, so a
profile written before it reads as opaque. The view turns it into an absorption per mm.

**A clear resin is drawn against a thickness pass**: before the frame, every model surface
adds its signed distance from the eye, measured from the pivot, into an R16Float plane; the
front faces are then drawn with the light behind kept by Beer-Lambert over that thickness.

**Creases are shaded in the copy pass, from the depth plane, only while the camera rests.**
The depth plane is kept past the scene pass for it; a moving camera keeps the plain copy.

On a WebGL canvas the view draws opaque and unshaded: neither pass reads a plane there.

## Consequences

The resin format gains a field; a reader that predates it ignores it. The clear view costs a
second draw of every model and a window-sized half-float plane, made on first use; the
shading costs sixteen depth reads a pixel on a resting frame. The command line has nothing
to show the view in, so it has no flag; it would need one if the view ever writes a picture.
Reopen if a GPU without blendable R16Float or depth sampling turns up outside WebGL.

## Alternatives considered

### AR through an exported GLB or USDZ

Shows size at true scale on a phone, but only there, and needs a mesh writer and its format
decisions first. References of known size answer the same question on every screen.

### Image-based lighting from HDR environments

The look of a product shot, for megabytes of assets and a prefiltering step; a sky gradient
and two softboxes in the shader cost nothing to ship.

### Order-independent transparency per surface

Would show every wall as its own pane, but reads the cavity of a hollowed model no better
than a thickness does, and costs a sort or several layers of planes.

### The option that won, and what it costs

Thickness from one plane is exact for closed meshes only: an open or inverted mesh adds up
to nonsense, clamped at zero. Shading creases from depth alone misses what the camera cannot
see and grains at sixteen samples, accepted for no blur pass.

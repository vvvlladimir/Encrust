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
steps the layer height into the surface in the shader, puts the printer away for a studio
room worked out per pixel, and stands the models on a cutting mat among a ruler, a pen, a
one-euro coin, a 330 ml can and a banana, laid round them at their real size, built from
primitives and painted by material in the shader. The key light casts their shadows through
a depth map drawn from it, only when what casts it changes. Picking,
slicing and the written file do not change.

**A resin carries `details.clarity`, 0.0 opaque to 1.0 water-clear**, default 0.0, so a
profile written before it reads as opaque. The form edits it as the opacity of the resin's
colour, in one picker with a `#RRGGBBAA` field; the view turns it into an attenuation distance.

**A clear resin refracts what stands behind it**, after glTF's `KHR_materials_volume`:
before the frame, every model surface adds its signed distance from the eye, measured from
the pivot, into an R16Float plane; everything else is drawn and copied out; the models' front
faces then read that copy along the view ray refracted by the resin and carried as far as the
part is thick, per channel, bent again where it leaves by the nearest back face a pass ahead
keeps, tinted by the resin's colour and clouded over that thickness and by the haze of
curing. The layers make the surface: a stair that bends the light in bands where they are a
few pixels apart, and finer than that a roughness that spreads it across the layers only.
The scene pass ends by laying every face of the clear models into it faintly, so walls
inside a hollow read through.

**Creases are shaded in the copy pass, from the depth plane, only while the camera rests**,
by ground-truth ambient occlusion. The depth plane is kept past the scene pass for it; a
moving camera keeps the plain copy.

**The light is summed in linear light and handed to AgX**, its studio a procedural room baked
once into a prefiltered map (Karis's split sum), so reflections roughen with the surface;
thin parts glow with the key behind them.

**A picture at rest is averaged over sixteen jittered frames**, then held until anything it
shows changes; that average is also the denoise of the creases, the frost and the soft
shadows. With *Depth of field* each frame is drawn from another point of a lens focused on
the models, so the set blurs with true occlusion and no post-process.

On a WebGL canvas the view draws opaque and unshaded, with no key shadow, average or lens:
none of these passes reads a plane there.

## Consequences

The resin format gains a field; a reader that predates it ignores it. The clear view costs
four draws of every model (ADR 0391), a copy of the colour plane, and window-sized
half-float planes, made on first use and let go when the view is off; the crease shading
costs 29 depth reads a pixel on a resting frame, and the average one more RGBA16Float plane.
The command line has nothing to show the view in, so it has no flag; it would need one if
the view ever writes a picture. Reopen if a GPU without blendable R16Float or depth sampling
turns up outside WebGL.

## Alternatives considered

### AR through an exported GLB or USDZ

Shows size at true scale on a phone, but only there, and needs a mesh writer and its format
decisions first. References of known size answer the same question on every screen.

### Photographed models of the desk's things

The look of scanned props, for a model format reader, compressed textures and megabytes in
the repository and the web build; procedural shapes and materials ship nothing. Kept open: a
loader can come later behind its own decision, once the files are chosen.

### Image-based lighting from HDR environments

The look of a product shot, for megabytes of assets; a sky and three softboxes worked out in
the shader and prefiltered once on the card ship nothing.

### Order-independent transparency per surface

Would show every wall as its own pane, but reads the cavity of a hollowed model no better
than a thickness does, bends nothing, and costs a sort or several layers of planes.

### Ray tracing the part

The true picture, two bends and caustics included, for a BVH on the card and a frame cost
no WebGL canvas and few laptops afford.

### The option that won, and what it costs

Thickness from one plane is exact for closed meshes only: an open or inverted mesh adds up
to nonsense, clamped at zero. Refraction is read off one copy of the scene, so it bends
only what is on screen and never one clear part seen through another, and it bends at the
near face and at the first back face, not at every wall after. Shading creases from depth
alone misses what the camera cannot see, and its grain waits on the average to go.

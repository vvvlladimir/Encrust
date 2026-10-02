# 0070. Draw a mesh in pieces the card will take

- **Status:** Accepted
- **Date:** 2026-09-22

## Context

The viewport uploads a mesh flat-shaded: three unshared vertices per face at 24 bytes
each, one vertex buffer per mesh, cached by the `Arc`'s address. That held while a mesh
was an imported model.

A hollowed model is not. A 406k-face figure with a 0.3 mm wall comes back as 11 506 949
triangles, which is 828 MB in one buffer. `wgpu::Limits::default().max_buffer_size` is
256 MiB and `encrust-app` asks for no limits, so the window panicked on the upload for any
mesh past 3 728 270 faces — with the field itself well inside its memory budget. The
crash was not in hollowing at all.

Raising the requested limit only moves the number: the ceiling is the adapter's, it
varies by machine, and the peak is doubled anyway by the vertex list held to hand the
buffer over.

## Decision

A cached mesh is a list of pieces, not one buffer. `pieces` divides the faces by the
device's own `max_buffer_size`, expands and uploads each piece on its own, and the draw
call is issued once per piece against the same instance slot.

## Consequences

No mesh size can fail the upload, on any adapter. Peak memory during the upload is one
piece rather than the whole mesh. Draw calls grow by one per 3.7M faces, which is
nothing beside the vertex count itself.

It does not make the cavity smaller: 828 MB still reaches the card, just legally. The
signal to reopen is the frame time of a hollowed plate — if it drops, the fix is fewer
triangles out of `extract`, not more buffers.

## Alternatives considered

### Ask for a bigger `max_buffer_size`

One line in the eframe configuration. Rejected because the real ceiling belongs to the
adapter, so the panic would come back on a smaller card instead of being gone.

### Draw the model and keep the cavity off the card

The cavity faces inward and is culled from outside, so it draws nothing for its 828 MB.
Rejected because the section cut counts its crossings in the stencil plane (ADR 0062) and
would cap the cut wrongly without it, and because caching two meshes per object where one
was cached costs more memory than it saves.

### The option that won, and what it costs

Pieces make an unbounded mesh legal, which is not the same as making it wise. The window
now quietly holds most of a gigabyte of GPU memory for a surface nobody looks at, and the
setting that got it there is no longer refused anywhere. Making `extract` emit fewer
triangles is still owed.

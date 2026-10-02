# Design notes

How things work: algorithms, maths, data layouts. One file per topic, named after the
topic (`slicing.md`, `contour-stitching.md`, `support-generation.md`).

This is where material goes when it is too long to be a comment. Code points here with a
single line; see `.claude/rules/code-style.md`.

A design note explains **how**. If you are explaining **why one option was chosen over
another**, that is an ADR and belongs in `docs/decisions/`.

| File | Topic |
|---|---|
| [mesh-repair.md](mesh-repair.md) | Welding, topology diagnostics, orientation |
| [slicing.md](slicing.md) | Plane intersection, degenerate cases, contour stitching |
| [rasterisation.md](rasterisation.md) | Pixel mapping, fill rule, coverage anti-aliasing |
| [viewport.md](viewport.md) | Plate coordinates, orbit camera, the wgpu frame, vertex layouts |
| [preview.md](preview.md) | The preview stack, shrinking a layer to a texture, what the panel reports |
| [thumbnail.md](thumbnail.md) | The view, the depth buffer and the resampling behind a sliced file's previews |
| [profiles.md](profiles.md) | The profile catalogue, the user directory, and one resin retuned per printer |
| [ui-design-system.md](ui-design-system.md) | The Encrust tokens, the widget set and the window's layout |
| [distance-queries.md](distance-queries.md) | The nearest point on a mesh, and the winding number that signs it |
| [volume.md](volume.md) | The distance field, its sparse storage, the CSG operators and extraction |
| [hollowing.md](hollowing.md) | The three hollow modes, the blockers, and the lattices that fill a cavity |
| [supports.md](supports.md) | Where a column can stand, the shape it is meshed as, placing one by hand |
| [analysis.md](analysis.md) | A layer's pieces from its runs, the pull on the film after Stefan, islands and levers |
| [hostile-files.md](hostile-files.md) | What a reader believes of a header, and the three guards under every count |
| [updates.md](updates.md) | The release feed, what an archive passes before it is unpacked, and the swap |

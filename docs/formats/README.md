# File formats

One file per format or family, named after it (`goo.md`, `chitu.md`, `anycubic.md`).

Each records the binary layout, field by field, with the offsets and types; where the
knowledge came from (an official specification, an open-source reader, observation of
real files); and every place our reading differs from the specification, with the reason.

Every format here is written and read back: `core-format` holds both halves, and step 18d1
added a reader beside each writer (ADR 0149).

Sources we use:

- the official Elegoo `.goo` specification;
- open-source readers of the containers that have no published specification;
- files written for real machines, read byte by byte.

When code has to do something unobvious because a format demands it, the code carries a
one-line pointer here and the explanation lives in this directory.

`sdcp.md` and `prusalink.md` are the exceptions to the first line: not containers but the
network protocols a file travels to a printer over, recorded here because they are read the
same way — an external specification, with every place a real machine differs from it.

`goo.md` covers the Elegoo `.goo` container, which step 4 writes. `chitu.md` covers the
Chitu family: `.ctb` versions 4 and 5 from step 8a, `.cbddlp` and `.photon` from step 18a.
`anycubic.md` covers the Photon Workshop family from step 18b, `sl1.md` the Prusa archive
from step 18c, and `gcode-zip.md` the `.zip` of greyscale PNGs from step 21d.
`creality.md` covers the `.cxdlp` at both its revisions from step 21e, `svgx.md` the
`.svgx` of polygons from step 21f, and `cws.md` the `.cws` archive from step 21g.
`encrust-project.md` covers our own `.encrust` project, which is not an external format and
so records its shape rather than an argument with a specification.

# 0366. Lay a file's plate out as known files do, and name the panel by the maker's mirror

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

ADR 0134 writes every mask as the plate stands, row 0 at its near edge, and leaves the
profile's `mirror_x` and `mirror_y` to the header. Twenty known files of one asymmetric
part, at one offset from the plate's centre, disagree. On every machine whose maker's settings state the panel as an
X-mirrored LCD (`.ctb`, `.goo`, `.fdg`, `.zip`, Photon Workshop, `.pm4u`) the known layer
is ours turned half round. On every other machine (`.ctb` Sonic Mega 8K S, `.phz`, `.cxdlp`)
and in `.photons` it is ours flipped in Y. `.cxline` matches as it is.

`.ctb` is in both groups, so the pixels follow the machine, not the container: the mirror is
drawn into the mask, not left to the header. The maker's writer makes one exception by name:
for the ELEGOO Saturn 3 Ultra, Jupiter SE, Saturn 4 Ultra and Mars 5 Ultra it reverses the
mask's rows instead of each row, and their known files hold the plate as it stands.

The makers' settings carry the mirror as one three-way choice — normal, LCD mirror, DLP
normal — that two independent flags only approximate.

## Decision

`PrinterProfile::mirror` is a `Mirror`: `normal`, `lcd-mirror` or `dlp-normal`, transcribed
from the maker's word. A profile carrying the old `mirror_x` reads it as `lcd-mirror` when
set and `normal` otherwise; `mirror_y` is dropped.

`core_pipeline::file_layout` is the one rule for every file written, converted or opened as
a model: rows from the far edge of the plate, mirrored along X on an `lcd-mirror` panel, a
half turn in all. The four machines above keep the plate as it stands on an `lcd-mirror`
panel, matched by `machine_name` without case as the maker's writer does. `.photons` mirrors
nothing along X and `.cxline` keeps the plate as it stands. `dlp-normal` is laid out as `normal`. The preview, the measuring and the PNG stack
still see the plate as ADR 0134 lays it. A header that states a mirror states `lcd-mirror`.

## Consequences

A part prints in the corner it was put in on every machine a known file exists for, and a
file of ours reads like the known one: twenty vendor samples of one part at one offset match
ours in full frame, across thirteen containers. The maker's `Left` is now the plate's `-X` edge on
every panel (ADR 0369). `convert` mirrors a layer along X, Y or both where the two containers
lay the machine's plate out differently.

Nine FlashForge and Nova3D profiles stated `mirror_y` alone, from no known file; they are
`normal` now. `.sl1` and the `dlp-normal` machines follow the rule with no known file either.
Reopen on a known file this rule does not reproduce.

## Alternatives considered

### Keep the plate as it stands on an X-mirrored machine

Such a part prints the right way round, only in the corner opposite the known file's, and
no file that already printed changes. It contradicts every
known file of the most common machines, and a user who lays a plate out by the maker's
picture finds the part elsewhere.

### Keep two flags

`mirror_x` and `mirror_y` admit a Y mirror alone and both at once, which no maker states,
and say nothing of DLP; transcribing a machine meant translating its word.

### The option that won, and what it costs

Every X-mirrored file changes, so a file written before this lands half turned when opened
as a model. The row and column order of a file depends on its machine and container, which a
reader of our files has to know to put the part back on the plate.

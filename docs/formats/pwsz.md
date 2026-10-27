# The PWSZ archive

What the newer Anycubic machines read: a plain zip of JSON settings, two binary entries and
one entry per layer. It is not the table container of `docs/formats/anycubic.md`, but its
layers are that container's PW0 runs, so it is written by the same crate (ADR 0361).

| Extension | Machine |
|---|---|
| `.pwsz` | Photon Mono M7 Pro |
| `.pm7` | Photon Mono M7 |
| `.pm7m` | Photon Mono M7 Max |
| `.pm4u` | Photon Mono 4 Ultra |
| `.pp1m` | Photon P1 Max |

The Photon P1 reads it too, as `.pp1`, but it has two panels side by side, and a profile
states one; it waits for a profile that can say two.

## Where the knowledge comes from

No file a machine accepted has been read, and none of these machines is in the room.

| Source | What it settles |
|---|---|
| The vendor's own slicer's writer, read statically | Which entries it writes and in what order, the keys it sets in each and from which setting, that a layer is `layer_N.pw0Img` in the PW0 codec, the scene head (388 bytes), every field of `calc_layer_volumes.data`, and that the machine settings are a template with the panel's fields written over |
| The same program's machine templates | Every constant below: the previews' sizes and colours, the firmware's motion and exposure constants, the resin's depth curve |
| An open-source reader and writer of the archive | The field order of a `scene.slice` row, which the vendor's slicer fills only in part |

Both writers write the layers in PW0, though the settings call the codec `pwszImg`: that
name is a vector codec of contour segments, which neither writes.

## Entries

Every entry is stored, not deflated: the layers are run-length coded already and the
previews are PNGs. The vendor writes the layers last; we write them as they come and the
entries that sum them up after, and a reader finds an entry by name.

| Entry | What it holds |
|---|---|
| `preview_images/preview_0.png` | 224 × 168, padded with `#024864` |
| `preview_images/preview_1.png` | 336 × 252, padded with `#141B29` |
| `software_info.conf` | `mark`, `os` and `version`: which program wrote it; `opengl` is `3.3-CoreProfile`, as the vendor states |
| `layer_images/layer_N.pw0Img` | layer `N`, from 0, in the PW0 codec, nothing in front |
| `layers_controller.conf` | `count`, then per layer `exposure_time` s, `layer_index`, `layer_minheight` mm (its floor), `layer_thickness` mm, `zup_height` mm, `zup_speed` mm/s |
| `print_info.json` | `cost`, `currency` (always `$`, as the vendor writes it), `print_time` s, `volume` ml |
| `anycubic_photon_resins.pwsp` | the machine and its resin, below |
| `scene.slice` | the model's extent and each layer's, below |
| `calc_layer_volumes.data` | the area each layer cures, below |

## `anycubic_photon_resins.pwsp`

Two objects: `machine_type` and `machine_extern`, both at version `"3"` and `"2"`.

`machine_type` states the panel from the profile — `res_x`, `res_y`, `xy_pixel` and
`xy_pixel_y` in micrometres, `print_xsize`, `print_ysize`, `print_zsize` in millimetres, one
`child_screen` covering it — and `name`, the profile's machine name. `key_suffix` is the
template's, `key_image_format` is `pwszImg`, `max_file_version` 518. The vendor writes the M7
Pro's template for every machine but the P1 family, and so do we: its suffix stays `pwsz`
under `.pm7`, `.pm7m` and `.pm4u` alike. The P1 Max's own template has `pp1m`, adds
`rotate_z` 180 and `print_platform_zheights` `[0, 35]`, and gives the resin a second bottom
exposure and the rests around a lift, all of which we write for it alone.

`machine_extern` names the machine again as `alias`, carries the firmware's motion and
exposure constants unchanged from the templates, and one user resin, which is also the
active one: named `User@<resin>@ACF@normal_print` as the vendor names a user's resin, code
`"10"` (standard resin), and the job's exposure, bottom layers, light-off, lift, retract and
transition layers in `slicepara` and `slice_extpara`. Speeds are millimetres a second.
`multi_state_used` is 1, as the vendor's slicer always states it: stage 0 is the lift we plan,
stage 1 climbs nothing, and both come down at the retract speed, as the `EXTRA` block of
`anycubic.md` does. Stage 1 keeps stage 0's speed up, since a firmware estimating time
divides by it. `gray_level` states the edge blur again, as the vendor's slicer does.
`anti_count` and `blur_level` are the panel's anti-alias level and edge blur, as in the table
container's header.
`intelli_mode` is 1, the vendor's value for a user resin; what the firmware does with it is
not known.

## `scene.slice`

All little-endian.

| Offset | Field | What we write |
|---|---|---|
| 0x000 | mark, 16 bytes | `ANYCUBIC-PWSZ`, nul-padded |
| 0x010 | software, 64 bytes | `Encrust` and its version |
| 0x050 | binary type, version, slice type, model unit, `u32` | 3, 1, 0, 0 |
| 0x060 | point ratio, `f32` | 1 |
| 0x064 | layer count, `u32` | |
| 0x068 | x, y, z of the model's near corner, `f32` mm | from the panel's centre; z 0 |
| 0x074 | x, y, z of its far corner, `f32` mm | z the print's height |
| 0x080 | model stats, `u32` | 0 |
| 0x084 | 256 reserved bytes | zero |
| 0x184 | `<---`, then the row count `u32` | |
| 0x18C | one 64-byte row per layer | below |
| | `--->` | |

A row: the layer's top in mm, its cured area in mm², its lit box from the panel's centre
(left, top, right, bottom, mm), the number of separate pieces it cures as a `u32`, the area
of the largest in mm², and 32 zero bytes. A dark layer's box is zero.

The vendor's slicer leaves the model's extent and every box zero and counts each layer as one
piece as large as the layer; we state what the layers light, which a reader of either form
takes the same way.

## `calc_layer_volumes.data`

A 16-byte head — `0x20251024`, 0.2 as `f32`, a `u32` 1, the layer count — then 48 bytes a
layer: its number counted from 1, its top in mm, the sliced layer height in mm, 32 zero
bytes, and the area it cures in mm², `f32`, the quantity `.cxline` states per layer.

## What we do not write

- **The `pwszImg` vector codec**: no writer known to us uses it.
- **The Photon P1's two panels.**
- **`lcd_function.json`**, which the open-source writer adds and the vendor's does not.
- **A reader.** Encrust writes the archive and does not open it.

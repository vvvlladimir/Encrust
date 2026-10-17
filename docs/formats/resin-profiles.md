# Resin profiles from other programs

Two exported profiles are read into a resin (ADR 0295), told apart by the extension. Both
were mapped from files exported with a test resin whose every number was unique, checked
against what the exporting program's window showed. What either leaves out keeps our
default; a lift, speed, wait or power goes through the bounds of ADR 0293.

## `.cfgx`

JSON. Each setting is an object whose `currentvalue` is the value; an export carries only
that key, a program's own installed profiles add `unit`, `showlist` and visibility flags.

The resin is `material[current_material].slice[current_slice]`, in its sections `resin`,
`print` and `advanced`. `current_slice` is stale in real exports (6 with one slice), so an
index past the end names the first slice. The machine is `printerinfo.machinetype`, its
panel `printerinfo.resolutionx` and `resolutiony`.

| Ours | Key | Note |
|---|---|---|
| `name` | `resin.name` | starts with a line break, trimmed |
| `details.kind` | `resin.resintype` | free text |
| `density_g_cm3` | `resin.resindensity` | g/ml |
| `details.price` | `resin.resincost` / `resin.resincostmeasure` | price per that many units |
| `details.currency` | `resin.resinunit` | index into the format's list of 38 signs, `¥` first |
| `details.price_per` | `resin.resinmassunit` | 0 shown as kg, 1 taken as litre; the field's own list reads `L`, `Kg` |
| `details.color` | `resin.color` | integer RGB: 5154885 is `#4EA845` |
| `layer_height_mm` | `print.layerheight` | mm |
| `bottom_layers` | `print.bottomlayercount` | |
| `exposure_s`, `bottom_exposure_s` | `print.normalexposuretime`, `bottomlayerexposuretime` | s; the first is required |
| `transition_layers` | `print.transitionlayercount` | `transitionlayertype` 0 is linear, the only ramp we have |
| `waits.mode` | `print.printwaitmode` | 0 rests, 1 holds the light off |
| `waits.*` | `print.normalresttimebeforelift`, `…afterlift`, `…afterretract` | s; the `bottom…` twins are dropped, as in ADR 0293 |
| `light_off_delay_s` | `print.lightofftime` | s |
| `lift_distance_mm`, bottom | `print.normallayerliftheight`, `bottomlayerliftheight` | mm; also our retract distance |
| `lift_speed_mm_min`, bottom | `print.normallayerliftspeed`, `bottomlayerliftspeed` | mm/min |
| `retract_speed_mm_min`, bottom | `print.normallayerdropspeed`, `bottomlayerdropspeed` | mm/min |
| `light_pwm`, bottom | `advanced.normallightintensitypwm`, `bottomlightintensitypwm` | 0–255 |
| `compensation.shrink_*_pct` | `advanced.compensation_x/y/z` | %, on with `enable_scale_compensation` |
| `hole_offset_mm`, `outer_offset_mm` | `advanced.innercircleoffset`, `outtercircleoffset` | mm, on with `binnerouttercircleoffsetenable` |
| bottom offsets | `advanced.bottominnercircleoffset`, `bottomouttercircleoffset` | mm, on with `bbottominnerouttercircleoffsetenable` |
| `parity_offset_mm` | `advanced.circleparitydifference` | mm |
| `layer_time_s` | `advanced.layerprinttime` | s, only with `bprinttimecompensationenable` |

A machine that peels on its own side, a tilting vat, states its layer height as the lift and
0.05 mm/min as every speed; the bounds drop them. The second stage of a two-stage peel
(`…liftheight2`, `…speed2`, `…dropheight2`) has no place in a resin and is not read.

## Material `.ini`

`key = value` lines; `#` and `;` start a comment, `[…]` a section, and `nil` means not set.
A saved configuration holds the whole job, a material preset only the material's keys. A
key `material_ow_<key>` is the material's override of `<key>` and wins over it: the
archive written from the test file states `numFade = 10`, the override, not the
`faded_layers = 6` beside it.

| Ours | Key | Note |
|---|---|---|
| `name` | `sla_material_settings_id` | |
| `exposure_s`, `bottom_exposure_s` | `exposure_time`, `initial_exposure_time` | s; the first is required |
| `bottom_layers`, `transition_layers` | `faded_layers` | 1 and `faded_layers - 2`: the firmware's fade, see `sl1.md` |
| `layer_height_mm` | `layer_height` | mm, in a saved configuration only |
| `compensation.shrink_*_pct` | `material_correction_x/y/z` | a factor, 1 is 100 % |
| `bottom_outer_offset_mm` | `elefant_foot_compensation` | mm, negated: the first layers' outline moves in |
| `density_g_cm3` | `material_density` | g/ml |
| `details.price`, per litre | `bottle_cost` over `bottle_volume` | ml; no currency is stated, ours stays |
| `details.color` | `material_colour` | `#RRGGBB` |
| machine | `printer_settings_id`, else `printer_model` | blank in a configuration saved from the command line |
| panel | `display_pixels_x`, `display_pixels_y` | |

There is no lift, speed or light power, and no bottom-layer count beyond the fade's one: the motion keys (`tilt_*`,
`tower_*`, `delay_*`) are for the vendor's own tilting machines, two values each by the
area being exposed, and do not map to a peel.

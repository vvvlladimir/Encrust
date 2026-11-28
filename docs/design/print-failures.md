# After a failed print

What a failed print most likely came from, for `encrust diagnose` and the `report_print`
tool (ADR 0461). The map is `crates/encrust-cli/src/diagnose/failures.toml`; this page says
how it is read and how to improve it.

## Symptoms, not photos

A person, or a chat client looking at their photo, names what the print showed from a fixed
list: `off-plate`, `layer-split`, `hole`, `support-pulled`, `face-mark`, `resin-crack`. Each
has its meaning in the map, so a client chooses by the same words a person reads. No image
reaches the engine: a symptom is something a test can give, a photo is not.

## Causes, read against the check

Each symptom lists its causes, each with the step it belongs to (`pose`, `hollow`, `drains`,
`supports`, `repair`, `resin`, `after` for washing and curing) and the change to try, named
by the tool that makes it. A cause may carry a sign, something the `--json` report of the
file printed shows:

| Sign | Read from the report |
|---|---|
| `likely-fails` | `print.cured.likely_fails_layer` |
| `suction-cup`, `peel`, `lever`, `island`, `thin` | `print.cured.risks` of that kind (`island` counts `floating` too), with the first one's layer and place |
| `trapped-resin` | `slicing.trapped_pockets` and `trapped_mm3` |
| `hollow` | a model's `hollow.wall_mm` |
| `supported`, `unsupported` | support tips in the models' `supports.contacts`, or no model with supports |
| `no-room` | the models' `supports.no_room_for` |
| `open-mesh` | a model whose `repair.closed` is false |

A cause whose sign the report shows comes first, with what it shows; a cause with no sign
follows, as what no check can see — an exposure, a level plate. A cause whose sign the
report does not show is left out: no support marks without supports.

## Improving the map

A print that failed for a reason the map put low, or did not list, is a line changed in the
TOML: a cause moved up, a sign added, a change reworded. A new sign needs its reading in
`diagnose.rs`, and the map's tests fail on a sign nothing reads. Each change is checked by
the next prints, which `report_print` keeps in the job's `prints.jsonl`.

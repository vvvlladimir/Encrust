# Architecture

## The pipeline

```
 .stl  ──>  Mesh  ──>  Sliced  ──>  masks, a window at a time  ──>  .goo / .ctb
      core-mesh-io  core-slicer      core-raster            core-format + format-*
```

1. **Import.** `core-mesh-io` parses a file into a `core_geometry::Mesh` — vertices plus
   triangle indices, millimetres, model space — unwelded. `weld` gives it topology,
   `diagnose` reports what is wrong, `orient_outward` fixes the winding, and `fill_holes`
   with `remove_duplicate_faces` mend what is open or drawn twice when the user asks for
   it (ADR 0194, 0195). A file that carries UVs and an image brings them along in
   `Loaded`, for `core_volume::press` to turn into relief (ADR 0115, 0116).
   See `docs/design/mesh-repair.md`.
2. **Placement.** A `Transform` puts the mesh on the plate. Slicing works on transformed
   coordinates; the original is kept so placement stays editable.
3. **Slicing.** `core-slicer` cuts the triangles with a plane in the middle of each layer
   and stitches the segments into closed `Contour`s along shared edges. `Sliced` is the
   stack plus counters for what a broken mesh forced it to paper over.
   See `docs/design/slicing.md`.
4. **Rasterisation.** `core-raster` fills contours into `LayerRuns`: runs of one 8-bit
   grey, LCD reading order, positive winding fill, edges anti-aliased by area coverage.
   `LayerMask` is the same layer expanded to pixels, for PNGs and the preview.
   See `docs/design/rasterisation.md`.
5. **Output.** `core-engine` takes the plate — every model where it stands — bakes it into
   one mesh, plans its layers and hands it to `core-pipeline`, which folds each window
   into what it cures and streams it into a sink (ADR 0174, 0175). The writer comes from
   the output extension (ADR 0127). `core-format` holds what every sliced file shares, the PNG
   codec the archive containers need and the seven-bit run-length codec two families carry
   their layers in included (ADR 0168); `format-goo`, `format-chitu`, `format-anycubic`,
   `format-sl1`, `format-gcode-zip`, `format-creality`, `format-svgx`, `format-cws` and
   `format-ctb-encrypted` write the containers. The output
   extension picks the writer (ADR 0047).
   A `PrintJob` carries everything but the masks, which arrive one at a time through a
   seekable `LayerSink`, because a full stack does not fit in memory and a header pointing
   forward at a table has to be written twice (ADR 0010, 0012, 0045). Previews come from
   `core-thumbnail`, rendered on the CPU from the same meshes (ADR 0048). The CLI's PNG
   stack is a debug artefact, not a printer format. A `PrintJob`'s `ExposurePlan` gives
   bands of height their own exposure over the resin's (ADR 0090).

Supports and hollows are geometry, so they enter before step 3, not into the masks
(ADR 0026): `core-supports` finds where they go by running step 3 first and reading the
stack (ADR 0029), merges tips into trunks (ADR 0040), and meshes them into the model;
`core-volume` returns a hollowed model as the solid with its cavity, infill and drain
holes appended (ADR 0059, 0071). Whether the resin can then get out is read off the stack
by `core-supports`, beside the islands (ADR 0072).

Which of these steps a model goes through, in what order and with what settings, is a
recipe's: `core-recipe` reads it and decides each step, and the window and the command line
run the step as they run it by hand (ADR 0402, 0403). See `docs/design/recipes.md`.

What the masks cure is read off them as they are written, in the rasteriser's parallel
map: `core-analysis` measures each layer's runs — area, pieces, the pull on the film — and
folds the volume the header, the weight and the price are taken from (ADR 0163).

A written file is the end of the pipeline. Sending it to a printer starts from the path
alone: `net-sdcp` and `net-prusalink` take it and nothing else (ADR 0136), and the window
chooses between a file and a printer on the one button that writes (ADR 0138). The two
protocols meet in `printer-link`, an enum over what a destination can be that the window and
`encrust printer` both call; a browser downloads the file instead (ADR 0182).

## What each crate owns

What each crate is responsible for, and the names to look for first; the rest of its API is
its `lib.rs` re-exports and rustdoc. The graph they may depend on is below, and the rules
that bind it are in `.claude/rules/architecture.md`.

| Crate | Owns |
|---|---|
| `core-geometry` | Geometry every crate shares: `Mesh`, `Aabb`, `Transform`, `Bvh` and its queries, `Region`, `Adjacency`, `FastMap`/`FastSet`; repair (`weld`, `diagnose`, `orient_outward`, `fill_holes`, `mend`), cuts and splits, `surface_path`, `convex_hull`, and the primitives tests build from (`cuboid`, `ball`, `cylinder`, …). Its `lib.rs` re-exports are the catalogue |
| `core-mesh-io` | Model files read: `MeshLoader` over a path or a `ModelFile` into `Loaded`, by `StlLoader`, `ObjLoader` and `ThreeMfLoader`, with their textures, and `decode_picture` |
| `core-slicer` | A mesh cut into layers: `SliceSettings`, `LayerPlan` and `adaptive_plan`, `SliceEngine`/`PlaneSliceEngine` into `Sliced` `Layer`s of `Contour`s, `Windows` sampling a layer, `offset_contours` |
| `core-raster` | Contours into pixels: `Rasterizer`/`ScanlineRasterizer` into `LayerRuns` and `LayerMask`, `RasterSettings`, the anti-aliased `Grey`, `downsample`, and `rings_of`, a mask traced back into rings |
| `core-analysis` | What a layer cures and what it risks: `cure` into `Cured` `Piece`s, `pull_n`, `Measured` with its `Risk`s — islands, levers, peels, suction cups, thin pieces — `holds_for_lever` and `TipLoads` (ADR 0163, 0281) |
| `core-supports` | Where supports go and their meshes: `SupportPoint`, `SupportTree`, `columns`, `grow`, `generate_supports`, `mesh_trees`/`mesh_groups`; the rules a support edited by hand is held to (`carried`, `allowed`, `Refused`); `Blocked`, `Shown`, `ModelSupports`; `TrapScan` for resin with no way out (ADR 0026, 0029) |
| `core-plate` | A model's pose and the plate's layout: `orient` and `orient_printed` with `OrientSettings`, `finalists`, `rank`/`rulings`; `PlateFit` and `crossings` between models; `arrange`, `nearest_free`, `move_clear`; `Watch`, a search's progress and cancel (ADR 0275, 0384) |
| `core-csg` | `Solid` — a closed mesh readied once for `union`, `difference` and `intersection`, exact, over `boolmesh` — and `CsgError` (ADR 0303) |
| `core-joint` | Joints for a cut: `Joints` of a `JointKind` — dowels, pins, magnets, a key — sized by `Joints::asked` and built by `join` into a `Joined` (ADR 0306) |
| `core-text` | Text on a model: `Font`, `outline` into `Shape`s, `label` with `LabelSettings` into a `Label` (ADR 0305) |
| `core-volume` | Fields and what is built on them: `Sdf`/`VoxelGrid` with `build` and `extract`; `combine`, exact or through the field, `repair` and `wrap`; `hollow` with `HollowSettings` and infill; drain holes (`DrainHole`, `drill`, `holes_out`, `plugs`); `parts` and `part_off`; `press`, a relief from a picture or a painted `Region`; `StackField`; `hollowing_saves_mm3` (ADR 0059, 0071, 0304) |
| `printer-profiles` | Printers, resins, support profiles, presets and the user's recipes as data: `PrinterProfile`, `MaterialProfile`, `SupportProfile`, `Catalogue`, `ProfileStore`/`DirStore`, `ForeignResin` read from another program's export, `STANDARD_PRINTER` (ADR 0226, 0235) |
| `core-thumbnail` | `Thumbnail`, `Part`, `Look`, `ThumbnailSettings`, `RESIN`, `SUPPORT`, `render` (CPU only); `render_view` from a `View` with `Marker`s drawn over it |
| `core-samples` | `TestPrint` — detail, exposure, surface and fit — each built in code with the mark raised on it, and `stl`, its file (ADR 0461) |
| `core-format` | What every sliced file shares: `PrintJob` with its `ExposurePlan`, `SlicedFileWriter`, `LayerSink`, `FormatError`, the PNG and RLE7 codecs, the Chitu containers' common blocks, `Crc32`, and the bounds every reader puts a header through (ADR 0010, 0168, 0171) |
| `core-pipeline` | The write stage every front end shares (ADR 0127): `SlicedFormat` picked by extension, `write_to`/`write` streaming a stack into a sink, `measure` without writing, `convert` between containers, `Observer` for progress and stop, `PipelineError` (ADR 0175, 0180) |
| `core-engine` | The plate every front end runs (ADR 0174): `Model`/`Plate`, `bake` and `Run` writing or measuring it; the autopilot's decisions — `needs_repair`, `hollowing`, `stand_under`, `strengthen` (ADR 0402); drains (`suggest_drains`, `trapped_in`); Check's fixes (`island_floors`, `lever_holds`, `cup_vents`); `orient_for_print` proving poses, with `KeepClear`; `ShownAt`; `open_plate`, `file_stem`; and `project`, the `.encrust` manifest with `ObjectId`s and `insert` (ADR 0191, 0456) |
| `core-edit` | `Edit`, one action on a plate naming its object by `ObjectId` and its points in plate mm, and `defaults`, the numbers taken where none is typed (ADR 0456) |
| `core-document` | `Document`, the plate as `.encrust` holds it, and `apply`, the one way an `Edit` is carried out, into `Applied` or `ApplyError`; the cut (`cut`, `pieces`), `Body` and Arrange's `Standing`, which the window and the command line both run (ADR 0456) |
| `core-recipe` | Recipes: `Recipe` of `Step`s read from TOML, `validate`d into a `Plan`, `measure`d into `Metrics`, each step's `Decision` by `decide`, a plate step as `Edit`s by `edits_of`; `built_in` with `prepare`, `library`/`find`, `RecipeError` (ADR 0403, 0405) |
| `format-goo` | `GooWriter`, `Goo5Writer`, `GooReader` and the `.goo` codecs of revisions 3.0 and 5.1 |
| `format-chitu` | `CtbWriter`, `CtbVersion` v4/v5; `CbddlpWriter`, `CbddlpFlavour` and the eight-pass RLE1 codec (ADR 0146); `ChituReader` (ADR 0149); `FdgWriter`, `FdgReader` and the one-bit `.fdg` codec; `PhzWriter`, `PhzReader`: the same layout under the `.phz` header |
| `format-anycubic` | `AnycubicWriter`/`AnycubicReader` with `AnycubicFlavour`, `AnycubicVersion` and the PW0 codec, `PwsReader`, `PwszWriter`/`PwszReader` for the PWSZ archive, `PhotonsWriter`/`PhotonsReader` (ADR 0147, 0361) |
| `format-sl1` | `Sl1Writer`, `Sl1Reader`, `Sl1Flavour`: a zip of settings files and PNGs (ADR 0148) |
| `format-gcode-zip` | `GcodeZipWriter`, `GcodeZipReader`, `claims`: a zip of PNGs whose `run.gcode` is the program the board runs, every value of it per layer (ADR 0167) |
| `format-creality` | `CxdlpWriter`/`CxdlpReader` with `CxdlpVersion` 3 and 4, and `CxlineWriter`/`CxlineReader`, layers as polygons traced out of the mask (ADR 0168, 0362, 0413) |
| `format-svgx` | `SvgxWriter`, `SvgxReader`: a binary header, two bitmap previews and an SVG document whose layers are polygons traced out of the mask (ADR 0169) |
| `format-cws` | `CwsWriter`, `CwsReader`, `claims`: a zip of eight-bit PNGs, one `slice.conf` and the gcode program that runs them (ADR 0170) |
| `format-ctb-encrypted` | `CtbEncryptedWriter`, `CtbEncryptedReader`, `claims`: the encrypted `.ctb`, its settings enciphered under the one key the container has and signed (ADR 0298) |
| `net-sdcp` | `Printer` over SDCP: `discover`/`probe` over UDP, `Control` over a WebSocket or MQTT — pause, resume, stop, history, camera — `upload`, `picture`, `SdcpError` (ADR 0154, 0155, 0263) |
| `net-prusalink` | `Link` over PrusaLink: `probe`, `upload`, `start_print`, `status`, `job` with pause, resume and stop, `picture`, `PrusaLinkError` (ADR 0152, 0263) |
| `printer-link` | `Wire` over a board or a Prusa machine, `upload` with the board's pre-flight, `start_print`, `state` into one `State`, `watch` reporting it every period, `control` by `Command`, `history`, `picture` and `camera`, `scan` of both into `Found`, `SendError` (ADR 0182, 0262, 0263) |
| `encrust-cli` | `encrust` binary, a subcommand per module in `commands/` (ADR 0176): the run — `stage` onto a `core_document::Document`, each shaping flag an `Edit` (ADR 0458), `recipe` and `--prepare`, `pipeline` into a file or a PNG stack — and `estimate`, `render`, `measure`, `convert`, `printer`, `profiles`, `recipes`, `test-print`, `diagnose`, `batch`, `config`, `completions`; the benches `hollow-lab`, `orient-lab`, `support-lab` and `agent-lab` (ADR 0455, 0460). Its flags: `docs/cli.md` |
| `encrust-web` | The window's browser front end (ADR 0181): `start`, the window on a canvas, and `www/`, the page and its service worker (ADR 0183) |
| `encrust-mcp` | `encrust-mcp` binary, the MCP server over stdio a chat client starts (ADR 0455): `Server` and its tools, `Jobs` of numbered `Revision`s, each a `Recipe` turned into `encrust` flags and kept with its check; `Cli` running `encrust --json`; `Trace`; `Window`, handing the open window revisions and `Edit`s (ADR 0456); `Presence`, how the window lists it (ADR 0462) |
| `agent-link` | What `encrust-mcp` and the window say to each other and where they keep it (ADR 0455): the link's lines and answers (`Show`, `PlateLine`, `ApplyReply`, …), `WindowFile`, `AgentFile`, and the jobs' folders under `cache_dir` |
| `web-engine` | The browser's front end without a window (ADR 0177): `slice_project`, the bytes of a project into the bytes of a sliced file with no file system, thread or clock, and the `wasm-bindgen` exports of it; `www/` the page, its worker and the Node measurement |
| `xtask` | `xtask` binary: `web`, the browser build; `man`, the command line's man pages from its own clap definition (ADR 0183); `arch`, the dependency graph below against every crate's manifest; `mcpb`, the chat-app extension for a target (ADR 0455) |
| `encrust-app` | `encrust-gui` binary, the egui/wgpu window: `Scene`, `BuildPlate`, `OrbitCamera`, picking, gizmo, `History`, the tools and their jobs, `prepare` running a recipe, `link` taking what `encrust-mcp` hands over (ADR 0456, 0462), `Settings`, `prefs`, `project` — the `Scene` ↔ `Manifest` conversion — `updates`, `report`, `i18n` (ADR 0300), `files` and, in a browser, `web` (ADR 0181). Its screens: `docs/design/ui-screens.md` |

## The allowed dependency graph

Arrows point at what a crate may depend on. Anything not drawn is forbidden, and
`cargo xtask arch` fails if this block and the manifests disagree. Dev-dependencies are
not drawn: a test may reach for a fixture the crate itself must not.

```
encrust-app ──> core-engine, every core-*, printer-profiles, printer-link, agent-link,
               net-sdcp, net-prusalink,
               egui, eframe, egui-wgpu, wgpu, transform-gizmo-egui, meshoptimizer-rs,
               bytemuck, image, rfd, rayon, serde, serde_json, zip,
               ureq (with TLS at the desk), minisign-verify, tar, flate2, web-time,
               fluent-bundle, unic-langid, sys-locale, jiff, fontdb at the desk;
               in a browser wasm-bindgen, wasm-bindgen-futures, js-sys, web-sys;
               on macOS objc2-app-kit, objc2-foundation, raw-window-handle;
               winresource at build time, for the Windows icon
encrust-web ──> encrust-app, wasm-bindgen, wasm-bindgen-futures, web-sys, getrandom
encrust-cli ──> core-engine, core-document, core-edit, core-analysis, core-format, core-geometry, core-joint,
               core-mesh-io, core-pipeline, core-plate, core-raster, core-recipe, core-slicer,
               core-samples, core-supports, core-text, core-thumbnail, core-volume, printer-profiles,
               printer-link, net-sdcp, net-prusalink, rayon, clap, clap_complete, serde, serde_json,
               toml, indicatif, ctrlc, jiff
web-engine ──> core-engine, core-pipeline, wasm-bindgen
xtask ──> encrust-cli, toml, clap, clap_mangen, zip
encrust-mcp ──> agent-link, core-edit, rmcp, tokio, base64, serde, serde_json, toml;
               it runs `encrust` as a child (ADR 0455, 0456)
agent-link ──> core-edit, directories, serde, serde_json

core-engine ──> core-pipeline, core-plate, core-analysis, core-format, core-geometry,
               core-joint, core-raster, core-slicer, core-supports, core-thumbnail,
               core-volume, printer-profiles, rayon,
               serde, serde_json, zip, jiff
core-document ──> core-edit, core-engine, core-geometry, core-joint, core-mesh-io, core-plate, core-slicer,
               core-supports, core-text, core-volume, printer-profiles, rayon, thiserror
core-edit ──> core-engine, core-geometry, core-joint, core-volume, serde
core-recipe ──> core-edit, core-engine, core-geometry, core-plate, core-volume, serde, thiserror, toml
core-pipeline ──> core-analysis, core-format, core-geometry, core-raster, core-slicer,
               printer-profiles, every format-*, rayon
format-goo ──> core-format, core-raster, md-5
format-chitu ──> core-format, core-raster
format-creality ──> core-format, core-raster, core-slicer, glam
format-anycubic ──> core-format, core-raster, serde_json, zip
format-sl1, format-gcode-zip, format-cws ──> core-format, core-raster, zip
format-ctb-encrypted ──> core-format, core-raster, aes, cbc, sha2
format-svgx ──> core-format, core-raster, core-slicer, glam
printer-link ──> net-sdcp, net-prusalink, serde
net-sdcp ──> (nothing in this workspace; tungstenite, ureq, md-5, serde, if-addrs)
net-prusalink ──> (nothing in this workspace; ureq, md-5, serde)
core-format ──> core-raster, core-slicer, printer-profiles, core-thumbnail, png, zip
core-thumbnail ──> core-geometry
core-samples ──> core-geometry
core-raster ──> core-slicer
core-supports ──> core-slicer, core-raster, core-geometry, printer-profiles, rayon
core-analysis ──> core-raster
core-volume ──> core-csg, core-geometry, rayon
core-csg ──> core-geometry, boolmesh, rayon
core-text ──> core-geometry, rayon, ttf-parser
core-joint ──> core-csg, core-geometry, rayon, serde
core-plate ──> core-analysis, core-geometry, core-slicer, rayon
core-slicer ──> core-geometry, i_overlay
core-mesh-io ──> core-geometry, zip
printer-profiles ──> (nothing in this workspace)
core-geometry ──> (nothing in this workspace; rayon)
```

No front end names the crate that writes a container: the flavour a `SlicedFormat` carries
— `CtbVersion`, `CbddlpFlavour`, `AnycubicFlavour`, `AnycubicVersion`, `Sl1Flavour`,
`CxdlpVersion` — is re-exported by `core-pipeline`, which is what picks the writer anyway
(ADR 0209). Only a test that reads back a file it asked for takes `format-goo`, as a
dev-dependency.

Only the front ends — the two binaries and `encrust-web` — may depend on graphics crates.
Spanning core layers is what `core-pipeline` and `core-engine` are for, and each stays its own stage: `core-pipeline`
is handed a mesh and its windows and writes a printable file (ADR 0127), `core-engine` is
handed a plate and runs it down to that call (ADR 0174). Neither is a place to put what
two callers merely happen to share. `core-mesh-io`,
`core-slicer` and `core-volume` are peers and never reference each other. A new
sliced-file format is a new `format-*` crate beside `format-goo`, depending on
`core-format` and nothing else in the workspace; a format whose container is someone else's
may take that container's crate with it, as the archive formats take `zip` (ADR 0148).
`core-mesh-io` takes `zip` for the same container: it reads 3MF through it with `quick-xml`,
refusing one whose directory claims a part larger than memory before reading any (ADR 0171,
0395). A new printer network protocol is a new
`net-*` crate, taking the path of a file that is already written and depending on nothing
in the workspace at all — not even on another `net-*` crate (ADR 0136). Where two of them
have something in common, it is settled in `printer-link` above them by an enum over
destinations, not by a crate under them (ADR 0182).

## Why the graph is shaped this way

- Every core crate derives `serde` on the data types a project file carries, always on
  rather than behind a feature: `cargo test --workspace` has to cover them (ADR 0191).
- `core-geometry` and `printer-profiles` are leaves and stay dependency-light.
  `core-geometry` owns the workspace's hash (ADR 0064). `printer-profiles` embeds
  `assets/profiles/` through its build script; profile ids are file stems and are stable,
  since renaming one orphans a user's override (ADR 0049).
- `core-volume` owns the distance field and everything over it — hollowing, infill, drain
  holes and cuts are operators on a field, not crates of their own (ADR 0055–0060). A
  hollowed mesh is the model with its cavity appended: it gets sliced, never re-fielded.
- `core-csg` is the one place an exact boolean is done, so the engine under it can be
  replaced without touching a caller (ADR 0303). It sits on `core-geometry`, readies a
  solid on `rayon` (ADR 0314), and `core-volume` takes it to cut a lattice out of a
  cavity's mesh (ADR 0302).
- `core-joint` joins a cut's halves with exact booleans, so it sits on `core-csg`, which
  `core-geometry` under it cannot reach; `core-engine` takes it only for the `Joints` a
  project writes down (ADR 0306).
- `core-text` reads a font and nothing else, so the parser stays out of `core-geometry`,
  which every crate takes; the window joins a label to its model through `core-volume`
  (ADR 0305).
- `core-supports` sits above `core-slicer` and `core-raster` because it reads a `Sliced`
  stack onto the rasteriser's grid (ADR 0029, 0032), and reaches sideways only to the
  `printer-profiles` leaf (ADR 0027). The workspace has no polygon boolean dependency.
- `core-analysis` reads the stack as runs, not contours, so what it measures is what
  cures (ADR 0163).
- `core-plate` reads the slicer because the peel force it scores is a cross-section, not
  a normal (ADR 0087), and `core-analysis` for the law that section pulls by and the limits
  it is held to, so the search scores what the proof measures (ADR 0281). It packs footprints as bitmaps of its own rather than taking on a
  polygon boolean (ADR 0088).
- `core-format` reads `core-slicer` for the `LayerPlan` a written file records, and
  re-exports it, so a `format-*` crate gets the type without a dependency of its own
  (ADR 0091).
- `core-pipeline` stops at the write stage, and `core-engine` sits above it with the run
  that reaches it: bake the plate into one mesh with the resin's shrinkage applied, plan
  the layers, render the thumbnail, assemble the `PrintJob`. Both front ends had written
  that five times over (ADR 0174). Hollowing is still not there: the CLI drives it from
  flags and the window from tool state (ADR 0127), and a `Model` carries only its result
  (ADR 0129). Automatic supports and the orientation proved on them are, since proving a
  pose is a run of the plate: stand, cut, cure and read (ADR 0277).
- `core-engine` also owns the `.encrust` file, because a plate live and a plate written
  down are the same facts in two forms. It is data and (de)serialisation only, so the
  command line and a browser can open a project without a window (ADR 0174).
- No core crate renders through a GPU; `core-thumbnail` is CPU-only (ADR 0048).
- `net-sdcp` and `net-prusalink` sit beside the `format-*` crates and depend on nothing in
  the workspace: each is handed the path of a file that is already written. One protocol is
  one crate, with no shared crate and no sideways edge between them (ADR 0136), and both
  block rather than bringing an async runtime into a synchronous workspace (ADR 0137). What
  the two have in common is settled in `printer-link`, above both and under both binaries,
  by an enum over a destination rather than a trait over a client (ADR 0153, 0182).
- `net-sdcp` covers SDCP versions 3 and 1, because they are one protocol over two transports
  and a second crate would duplicate the envelope, the discovery parser and the ack table
  (ADR 0154). Version 1 inverts the roles, so the crate runs an MQTT broker and a one-file
  HTTP server, both written there rather than taken as dependencies (ADR 0155). This is the
  only code in the workspace that accepts an inbound connection.
- `encrust-app` is the only crate that turns TLS on in `ureq`: the update check is the one
  call that leaves the local network, and it is off until the user turns it on (ADR 0172).
- Every crate depends on `workspace-hack`, generated by `cargo hakari`: no code, only each
  third-party dependency at the one feature set the workspace uses, so a crate built alone
  reuses the same build of it. It lives outside `crates/`, draws no edge above, and leaves out
  the Objective-C runtime and `ureq` so neither reaches a crate that did not ask (ADR 0272).

## Key types

| Type | Crate | Notes |
|---|---|---|
| `Mesh` | `core-geometry` | `Vec<Vec3>` plus `Vec<[u32; 3]>` |
| `Aabb` | `core-geometry` | Re-exported from `parry3d`, not redefined |
| `Transform` | `core-geometry` | Scale, then rotation, then translation |
| `Ray` | `core-geometry` | Origin plus unit direction, so `t` is a distance |
| `MeshDiagnostics`, `Orientation` | `core-geometry` | Topology counts; what repair changed |
| `PlacedHit` | `core-geometry` | Where a ray met a transformed mesh, in the ray's space |
| `ClosestPoint`, `Winding` | `core-geometry` | Nearest surface, and which side of it (ADR 0053, 0054) |
| `UvMap`, `Heightmap` | `core-geometry` | Three coordinates a face, and the image they address (ADR 0115, 0116) |
| `Solid` | `core-csg` | A closed mesh as the boolean engine holds it, or nothing; never changed by an operation (ADR 0303) |
| `Sdf`, `VoxelGrid` | `core-volume` | Narrow band of 8³ tiles plus solid runs, on a global lattice (ADR 0057) |
| `FieldSettings`, `HollowSettings`, `InfillSettings`, `Hollowed` | `core-volume` | What to build, how thick, what stands in the cavity, what came out |
| `ReliefSettings` | `core-volume` | How deep a texture is pressed into the surface, and on what lattice (ADR 0116) |
| `Layer`, `Contour`, `Sliced` | `core-slicer` | One Z and its contours; winding says material or hole |
| `Run`, `LayerRuns`, `LayerMask`, `Rastered` | `core-raster` | Runs are what every format encodes (ADR 0020) |
| `Cured`, `Measured`, `Risk` | `core-analysis` | A layer's pieces and pull; the stack folded in order, and where it fails (ADR 0163, 0164) |
| `SupportPoint`, `Column`, `SupportTree` | `core-supports` | A contact in model space, resolved against the plate, merged into trunks |
| `Placed`, `Region`, `Blocked` | `core-supports` | The model a support lives with, a patch of its faces, and the patch supports keep off (ADR 0092, 0093) |
| `Profiles` | `core-supports` | The shape each group of supports is built to (ADR 0094) |
| `ModelSupports`, `ModelHollow` | `core-supports`, `core-volume` | What one model carries — points, paint, frozen trees; blockers, holes, channels, its shell — in its own space (ADR 0129) |
| `SupportProfile` | `printer-profiles` | Tip, top, middle, bottom, small pillar, branching, raft, bracing |
| `Thumbnail` | `core-thumbnail` | An RGB picture of the plate, cut to each format's records |
| `LayerPlan` | `core-slicer` | Where every layer starts and stops; one shape for a uniform stack and an adaptive one (ADR 0091) |
| `PrintJob`, `Fields` | `core-format` | Everything but the masks; a seekable sink in either endianness |
| `CtbVersion` | `format-chitu` | Which `.ctb` revision to write: 4 or 5 |
| `CbddlpFlavour` | `format-chitu` | Which extension the older container takes: `.cbddlp` or `.photon` |
| `AnycubicFlavour` | `format-anycubic` | Which of the twenty Anycubic extensions a file takes |
| `AnycubicVersion` | `format-anycubic` | Which Photon Workshop revision a file states: 1, 515, 516, 517 or 518 (ADR 0166) |
| `Sl1Flavour` | `format-sl1` | Which Prusa extension a file takes: `.sl1` or `.sl1s` |
| `SlicedFile` | `core-format` | What an opened file says about itself, whoever wrote it (ADR 0149) |
| `Opened<S>` | `core-pipeline` | An opened sliced file over the source it took, whichever container it turned out to be (ADR 0150) |
| `Plate`, `Model` | `core-engine` | What a run cuts: the models where they stand, the machine, the resin and the cutting (ADR 0174) |
| `BuildPlate` | `encrust-app` | The machine's platform, which is what a model is placed on |
| `Preview` | `encrust-app` | The layer under the slider, off a source that is a plate being cut or a file being read (ADR 0151) |

## Coordinates and units

- Millimetres in geometry, seconds for exposure, pixels only in `core-raster` and below.
- Z is up, the plate is `z = 0`. Layer `n` covers
  `[z_min + n·h, z_min + (n+1)·h)` and is sampled in the middle, so `z_max` is never cut.
- `Scalar` is `f32` (ADR 0003).
- Panel mirroring lives in `PrinterProfile::mirror`; `core_pipeline::file_layout` draws it
  into a written file's masks, never into the geometry (ADR 0366).
- Every height a file states is rounded to the micron the machines step in (ADR 0133).

## Inside encrust-app

```
main.rs      the one argument, the logger, and a call into the library
lib.rs       the modules, `run` — window options and the model to open on startup — and
             `run_web`, the same on a canvas
files.rs     Handed: a file the user gave, a path or its bytes; the dialogs that ask for one
app.rs       SlicerApp: mode, tool, and the four groups of state.rs
state.rs     Doc, View, Tools, Machine: the window state in the groups it travels in
tool_settings.rs
             ToolSettings: every value a tool panel holds, for prefs, project and undo
workspace.rs Mode, Tool, ViewOptions, Array
shortcuts.rs every key the window answers, and what each one does
scene.rs     Scene, SceneObject, ObjectId, ImportSummary, the plates, duplicate, mirror
undo.rs      History: scene snapshots and tool-value snapshots, found by watching both, and
             the hold that makes a run of landings one entry
measure.rs   the two picked points, and the corner a click snaps to
import.rs    the files being opened: load, repair, index, place
repair.rs    the question a broken model raises, and the runs that close its holes
plate.rs     BuildPlate: the build volume, named apart from core_engine::Plate
camera.rs    OrbitCamera and its matrices
pick.rs      cursor to ray, ray to nearest object
gizmo.rs     transform handles over transform-gizmo-egui
slicing.rs   the slicing settings the window carries, and the job they start
hollow.rs    the hollow tool's settings and its run
drain.rs     the drain tool, and `Traps`: what the last drainage check found in a model
orient.rs    the auto-orient run, and the turn it applies to the scene
prepare.rs   Prepare for print: the tools' jobs started in turn by `--prepare`'s decisions,
             held as one undo entry (ADR 0402)
arrange.rs   packing everything visible onto the plate
cut.rs       the cut plane, the halves it leaves, and splitting into parts
preview/     the sliced stack and the slider over it, the file it may be read from, the
             runs that cut and measure it, and the picture of one layer
project/     the .encrust dialogs, and the plate captured into and applied from
             core_engine::project
job/         worker threads: import, repair, merge, export, preview, measure, supports,
             hollow, orient, send
panels/      top bar, tool rail, inspector, plate column, stage and its layer strip
profiles.rs  loading a profile from a file dialog
network.rs   printers a scan found, where the Slice button sends, and the errand running
prefs.rs     the machine, resin, tool values and printer addresses remembered between runs
report.rs    the bug report the window writes, and the issue it is carried into (ADR 0212)
i18n.rs      the language the window is written in, its catalogues and `tr!` (ADR 0300)
web/         a browser's threads, dialogs, downloads, private storage and page storage
ui/          design tokens, fonts, icons, widgets
render/      wgpu resources and the frame they prepare, its pipelines, buffers and
             textures, the paint callback, shader.wgsl
```

Scene and camera are plain data owned by `SlicerApp`; panels borrow them through
`panels::Window`, one field per group. How a frame is
drawn: `docs/design/viewport.md`.

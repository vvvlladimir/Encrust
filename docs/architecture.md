# Architecture

## The pipeline

```
 .stl  ──>  Mesh  ──>  Sliced  ──>  masks, a window at a time  ──>  .goo / .ctb
      core-mesh-io  core-slicer      core-raster            core-format + format-*
```

1. **Import.** `core-mesh-io` parses a file into a `core_geometry::Mesh` — vertices plus
   triangle indices, millimetres, model space — unwelded. `weld` gives it topology,
   `diagnose` reports what is wrong, `orient_outward` fixes the winding. A file that
   carries UVs and an image brings them along in `Loaded`, for `core_volume::press` to
   turn into relief (ADR 0115, 0116).
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
5. **Output.** `core-pipeline` runs steps 4 and 5 for both front ends — it picks the
   writer from the output extension, folds each window into what it cures and streams it
   into the file (ADR 0127). `core-format` holds what every sliced file shares, the PNG
   codec the archive containers need and the seven-bit run-length codec two families carry
   their layers in included (ADR 0168); `format-goo`, `format-chitu`, `format-anycubic`,
   `format-sl1`, `format-gcode-zip`, `format-creality`, `format-svgx` and `format-cws`
   write the containers. The output
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

What the masks cure is read off them as they are written, in the rasteriser's parallel
map: `core-analysis` measures each layer's runs — area, pieces, the pull on the film — and
folds the volume the header, the weight and the price are taken from (ADR 0163).

A written file is the end of the pipeline. Sending it to a printer starts from the path
alone: `net-sdcp` and `net-prusalink` take it and nothing else (ADR 0136), and the window
chooses between a file and a printer on the one button that writes (ADR 0138). The two
protocols meet only in the window, as an enum over what a destination can be (ADR 0153).

## What each crate owns

The full list of public types and functions, so that "where does this live" is answered
here and not by grepping. The graph they may depend on is below, and the rules that bind
it are in `.claude/rules/architecture.md`.

| Crate | Owns |
|---|---|
| `core-geometry` | `Mesh`, `UvMap`, `Heightmap`, `Triangle`, `Aabb`, `Transform`, `Ray`, `PlacedHit`, `Bvh` with `closest`/`faces_within`, `ClosestPoint`, `Adjacency`, `Winding`, `Plane`, `Cut`, `FastHasher`/`FastMap`/`FastSet`, glam re-exports; `weld`, `diagnose`, `orient_outward`, `center_of_mass`, `transform_mesh`, raycasts, `closest_point`, `winding_number`, `cut`, `split` |
| `core-mesh-io` | `MeshLoader` returning `Loaded` with one `Texture` per material and `decode` to a `Heightmap`, `StlLoader`, `ObjLoader` with UVs and `map_Kd`, `ThreeMfLoader` with `texture2dgroup` |
| `core-slicer` | `SliceSettings`, `LayerPlan`, `AdaptiveSettings`, `Layer`, `Contour`, `Sliced`, `SliceEngine` with `slice_at`, `PlaneSliceEngine`, `layer_heights`, `adaptive_plan`, `offset_contours`, `Windows` sampling several planes a layer |
| `core-raster` | `RasterSettings`, `Grey`, `Run`, `LayerRuns` with `blurred`, `LayerMask`, `Rastered`, `Rasterizer`, `ScanlineRasterizer`, `downsample` |
| `core-analysis` | `cure` into a `Cured` layer of `Piece`s with area, centre and pull after Stefan; `Measured` folding volume, hardest pull, widest step and `Risk`s — islands, levers on the narrowest neck under a piece, peels — or taking islands out as it goes; `erase`, `island_runs`, `equivalent_disc_mm` |
| `core-supports` | `Placed`, `Profiles`, `SupportPoint`, `Landing`, `Column`, `SupportTree`; `columns`, `grow`, `mesh_trees`/`mesh_groups`, `grab`/`Grab`/`Part`, `support_under`, `generate_supports`; `Region`/`Blocked` and `project`/`ProjectSettings` for a painted patch; `ModelSupports`, the supports one model carries; `TrapScan`/`Trapped` for resin with no way out |
| `core-plate` | `OrientSettings`, `Oriented`, `Score`, `Footprint`, `ArrangeSettings`, `Arranged`, `Placed`, `PlateError`; `orient` — flat faces and a Fibonacci sphere scored on overhang, peel, height and footprint, the best few cut to measure the section; `arrange` — footprint bitmaps packed into the corner and centred |
| `core-volume` | `Sdf`, `VoxelGrid`, `FieldSettings`, `SignMode`, `VolumeError`; `build` — scattered from the faces, carried coarse across the wall, refined where it is stored — CSG operators, `extract` — clustered surface nets; `hollow` with `HollowSettings`, `HollowMode`, `Blocker`, `Hollowed`, `lattice_mm`, `MIN_WALL_MM`, `sleeves`, `InfillSettings`/`InfillPattern`; `DrainHole`, `Channel`, `drill`, `bores`, `channel_under`, `hole_at`, `lift_for`, `pierce`; `ModelHollow` with `Shell`, `HoleSize`, `markers` — what one model carries; `press` with `ReliefSettings`/`Relief` |
| `printer-profiles` | `PrinterProfile`/`OutputFormat`/`AnycubicExtension`/`PhotonRevision`/`Connection`/`Firmware`, `MaterialProfile`/`PrinterTuning`/`Compensation` and `exposure_for_mm`, `SupportProfile` and its segments, TOML load/save, `Catalogue` of printers, resins and support profiles, and `user_dir` |
| `core-thumbnail` | `Thumbnail`, `Part`, `ThumbnailSettings`, `render` (CPU only) |
| `core-format` | `PrintJob`, `ExposureRange`/`ExposurePlan`, `SlicedFileWriter`, `LayerSink`, `Fields`, `FormatError`, the greyscale and colour PNG codec the archive containers share (ADR 0167), and `Rle7Layer`/`decode_rle7` with the RGB15 preview record two binary families share (ADR 0168); `Reads::claim`, `panel_in_range` and `read_entry`, the bounds every reader puts a header's counts through (ADR 0171) |
| `core-pipeline` | The stage both front ends share (ADR 0127): `SlicedFormat` and the extension that picks it, `PanelOverrides`/`raster_settings`, `Folded`/`Tolerance`/`fold_group`, `Writing`/`Written`/`write` streaming a stack into a file, `measure` doing the same without writing, `Observer` for a window arriving, layers landing and whether to stop, `PipelineError` |
| `format-goo` | `GooWriter`, `GooReader` and the `.goo` codec |
| `format-chitu` | `CtbWriter`, `CtbVersion` v4/v5; `CbddlpWriter`, `CbddlpFlavour` and the eight-pass RLE1 codec (ADR 0146); `ChituReader` and `layer_crypt` (ADR 0149) |
| `format-anycubic` | `AnycubicWriter`, `AnycubicFlavour` over seventeen extensions, `AnycubicVersion` v1/516/517, `AnycubicReader` and the four-bit PW0 codec (ADR 0147, 0166) |
| `format-sl1` | `Sl1Writer`, `Sl1Reader`, `Sl1Flavour`: a zip of settings files and PNGs (ADR 0148) |
| `format-gcode-zip` | `GcodeZipWriter`, `GcodeZipReader`, `claims`: a zip of PNGs whose `run.gcode` is the program the board runs, every value of it per layer (ADR 0167) |
| `format-creality` | `CxdlpWriter`/`CxdlpReader` and `CxdlpVersion`: version 3's vertical lines and version 4's Chitu-shaped tables under one magic, with the checksum both end with (ADR 0168) |
| `format-svgx` | `SvgxWriter`, `SvgxReader`: a binary header, two bitmap previews and an SVG document whose layers are polygons traced out of the mask (ADR 0169) |
| `format-cws` | `CwsWriter`, `CwsReader`, `claims`: a zip of eight-bit PNGs, one `slice.conf` and the gcode program that runs them (ADR 0170) |
| `net-sdcp` | `Printer` with its `Transport`, `Attributes`, `Status`, `Machine`, `PrintInfo`, `FileTransferInfo`/`Fetching`, `Transfer`, `SdcpError`; `discover`/`probe` over UDP for both reply shapes, `Control` over a WebSocket or an MQTT broker of its own, `upload` posting packets or serving the file a board fetches (ADR 0154, 0155) |
| `net-prusalink` | `Link`, `Auth`, `Version`, `DEFAULT_USER`, `PrusaLinkError`; `probe`, `upload` as one PUT and `start_print`, over HTTP digest or an API key (ADR 0152) |
| `encrust-cli` | `slice` binary: `pipeline` runs one model — orient, hollow, supports, then cut and stream the stack a window at a time into `.goo`, `.ctb` or PNGs; `batch` runs it over a directory with a JSON report per model |
| `xtask` | `xtask` binary: `gen-profiles`, the printer catalogue transcribed from a directory of source profiles, run by hand and never from a build script (ADR 0165) |
| `encrust-app` | `encrust` binary: egui/wgpu window — plate panel left, one inspector panel per tool and the rail beside it, plate tabs on their own strip, Preview splitting the stage between model and mask; `Scene` with `duplicate`/`mirror`/`array`, `Plate`, `OrbitCamera`, picking, gizmo, `History`, `Measure`, `Cutting`, jobs that hold no stack, `Settings`, `shortcuts`, `ui/theme`, `prefs`, `project`, `updates` |

## The allowed dependency graph

Arrows point at what a crate may depend on. Anything not drawn is forbidden.

```
encrust-app ──> every core-*, printer-profiles, every format-*, net-sdcp, net-prusalink,
               egui, eframe, egui_dock, egui-wgpu, wgpu, transform-gizmo-egui,
               bytemuck, image, rfd, rayon, serde, serde_json, zip,
               ureq (with TLS), minisign-verify, tar, flate2
encrust-cli ──> every core-*, printer-profiles, every format-*, rayon
xtask ──> printer-profiles, toml

core-pipeline ──> core-analysis, core-format, core-geometry, core-raster, core-slicer,
               printer-profiles, every format-*, rayon
format-goo, format-chitu, format-anycubic, format-creality ──> core-format, core-raster
format-sl1, format-gcode-zip, format-cws ──> core-format, core-raster, zip
format-svgx ──> core-format, core-raster, core-slicer, glam
net-sdcp ──> (nothing in this workspace; tungstenite, ureq, md-5, serde)
net-prusalink ──> (nothing in this workspace; ureq, md-5, serde)
core-format ──> core-raster, core-slicer, printer-profiles, core-thumbnail, png
core-thumbnail ──> core-geometry
core-raster ──> core-slicer
core-supports ──> core-slicer, core-raster, core-geometry, printer-profiles, rayon
core-analysis ──> core-raster
core-volume ──> core-geometry, rayon
core-plate ──> core-geometry, core-slicer, rayon
core-slicer ──> core-geometry, i_overlay
core-mesh-io ──> core-geometry, zip
printer-profiles ──> (nothing in this workspace)
core-geometry ──> (nothing in this workspace)
```

Only the two binaries may depend on graphics crates. Spanning core layers is what
`core-pipeline` is for, and it stays the write stage: a cut stack into a printable file,
never a place to put anything else two callers happen to share (ADR 0127). `core-mesh-io`,
`core-slicer` and `core-volume` are peers and never reference each other. A new
sliced-file format is a new `format-*` crate beside `format-goo`, depending on
`core-format` and nothing else in the workspace; a format whose container is someone else's
may take that container's crate with it, as the archive formats take `zip` (ADR 0148).
`core-mesh-io` takes `zip` for the same container, to refuse a 3MF whose directory claims a
part larger than memory before the loader under it reserves one (ADR 0171). A new printer network protocol is a new
`net-*` crate, taking the path of a file that is already written and depending on nothing
in the workspace at all — not even on another `net-*` crate (ADR 0136). Where two of them
have something in common, it is settled in `encrust-app` by an enum over destinations, not
by a shared crate (ADR 0153).

## Why the graph is shaped this way

- Every core crate derives `serde` on the data types a project file carries, always on
  rather than behind a feature: `cargo test --workspace` has to cover them (ADR 0097).
- `core-geometry` and `printer-profiles` are leaves and stay dependency-light.
  `core-geometry` owns the workspace's hash (ADR 0064). `printer-profiles` embeds
  `assets/profiles/` through its build script; profile ids are file stems and are stable,
  since renaming one orphans a user's override (ADR 0049).
- `core-volume` owns the distance field and everything over it — hollowing, infill, drain
  holes and cuts are operators on a field, not crates of their own (ADR 0055–0060). A
  hollowed mesh is the model with its cavity appended: it gets sliced, never re-fielded.
- `core-supports` sits above `core-slicer` and `core-raster` because it reads a `Sliced`
  stack onto the rasteriser's grid (ADR 0029, 0032), and reaches sideways only to the
  `printer-profiles` leaf (ADR 0027). The workspace has no polygon boolean dependency.
- `core-analysis` reads the stack as runs, not contours, so what it measures is what
  cures (ADR 0163).
- `core-plate` reads the slicer because the peel force it scores is a cross-section, not
  a normal (ADR 0087). It packs footprints as bitmaps of its own rather than taking on a
  polygon boolean (ADR 0088).
- `core-format` reads `core-slicer` for the `LayerPlan` a written file records, and
  re-exports it, so a `format-*` crate gets the type without a dependency of its own
  (ADR 0091).
- `core-pipeline` sits above every other core because it is the one place that runs them
  in order. It stops at the write stage: orient, hollow and supports are driven from
  flags in the CLI and from tool state in the window, so they were never duplicated and
  never moved (ADR 0127).
- No core crate renders through a GPU; `core-thumbnail` is CPU-only (ADR 0048).
- `net-sdcp` and `net-prusalink` sit beside the `format-*` crates and depend on nothing in
  the workspace: each is handed the path of a file that is already written. One protocol is
  one crate, with no shared crate and no sideways edge between them (ADR 0136), and both
  block rather than bringing an async runtime into a synchronous workspace (ADR 0137). What
  the two have in common is settled in `encrust-app`, where both are already linked, by an
  enum over a destination rather than a trait over a client (ADR 0153).
- `net-sdcp` covers SDCP versions 3 and 1, because they are one protocol over two transports
  and a second crate would duplicate the envelope, the discovery parser and the ack table
  (ADR 0154). Version 1 inverts the roles, so the crate runs an MQTT broker and a one-file
  HTTP server, both written there rather than taken as dependencies (ADR 0155). This is the
  only code in the workspace that accepts an inbound connection.
- `encrust-app` is the only crate that turns TLS on in `ureq`: the update check is the one
  call that leaves the local network, and it is off until the user turns it on (ADR 0172).

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
| `AnycubicFlavour` | `format-anycubic` | Which of the seventeen Anycubic extensions a file takes |
| `AnycubicVersion` | `format-anycubic` | Which Photon Workshop revision to write: 1, 516 or 517 (ADR 0166) |
| `Sl1Flavour` | `format-sl1` | Which Prusa extension a file takes: `.sl1` or `.sl1s` |
| `SlicedFile` | `core-format` | What an opened file says about itself, whoever wrote it (ADR 0149) |
| `Opened<S>` | `core-pipeline` | An opened sliced file over the source it took, whichever container it turned out to be (ADR 0150) |
| `Preview` | `encrust-app` | The layer under the slider, off a source that is a plate being cut or a file being read (ADR 0151) |

## Coordinates and units

- Millimetres in geometry, seconds for exposure, pixels only in `core-raster` and below.
- Z is up, the plate is `z = 0`. Layer `n` covers
  `[z_min + n·h, z_min + (n+1)·h)` and is sampled in the middle, so `z_max` is never cut.
- `Scalar` is `f32` (ADR 0003).
- Panel mirroring lives in `PrinterProfile`, reaches the file header alone, and is never
  applied to a mask or to the geometry (ADR 0134).
- Every height a file states is rounded to the micron the machines step in (ADR 0133).

## Inside encrust-app

```
main.rs      the one argument, the logger, and a call into the library
lib.rs       the modules, and `run` — window options and the model to open on startup
app.rs       SlicerApp: mode, tool, and the four groups of state.rs
state.rs     Doc, View, Tools, Machine: the window state in the groups it travels in
workspace.rs Mode, Tool, ViewOptions, Array
shortcuts.rs every key the window answers, and what each one does
scene.rs     Scene, SceneObject, ObjectId, ImportSummary, the plates, duplicate, mirror
undo.rs      History: whole-scene snapshots, found by hashing what can be edited
measure.rs   the two picked points, and the corner a click snaps to
import.rs    the files being opened: load, repair, index, place
plate.rs     the build volume
camera.rs    OrbitCamera and its matrices
pick.rs      cursor to ray, ray to nearest object
gizmo.rs     transform handles over transform-gizmo-egui
slicing.rs   the slicing settings the window carries, and the job they start
hollow.rs    the hollow tool's settings and its run
drain.rs     the drain tool, and `Traps`: what the last drainage check found in a model
orient.rs    the auto-orient run, and the turn it applies to the scene
arrange.rs   packing everything visible onto the plate
cut.rs       the cut plane, the halves it leaves, and splitting into parts
preview.rs   the sliced stack, the texture of one layer, and what the stack cures
project/     the .encrust file: manifest, mesh blob, the plate captured and applied
job/         worker threads: import, merge, export, preview, measure, supports, hollow, orient, send
panels/      title strip, tool rail, stage, inspector, status strip
profiles.rs  loading a profile from a file dialog
network.rs   printers a scan found, where the Slice button sends, and the errand running
prefs.rs     the machine, resin, format and printer addresses remembered between runs
ui/          design tokens, fonts, icons, widgets
render/      wgpu pipelines, buffers, paint callback, shader.wgsl
```

Scene and camera are plain data owned by `SlicerApp`; panels borrow them through
`panels::Window`, one field per group. How a frame is
drawn: `docs/design/viewport.md`.

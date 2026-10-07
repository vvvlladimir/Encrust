# Architecture decision records

One decision per file: what forced it, what was decided, what it costs. It exists so the
reasoning does not have to be reconstructed from the code, and so a decision is revisited
deliberately rather than drifted away from.

## Rules

- `NNNN-<slug>.md`, numbered from `0001`, never reused. A number may be missing: a record
  whose decision was replaced outright is removed once its successor carries the argument.
  `0000-template.md` is the template, not a decision.
- Write one when a change adds, removes or re-scopes a crate; picks a dependency that
  would hurt to swap; fixes a data format, convention or unit; chooses an algorithm
  family; or changes a public trait other crates implement. Not for a bug fix, a
  behaviour-preserving refactor, or a choice with an obvious default.
- **A decision that does not leave its crate does not come here.** If it changes no public
  API, no data format and no dependency, it is rustdoc on the item or a page in
  `docs/design/`. Several decisions taken together in one step are one record, not one
  each.
- Short: argument only, a few lines per heading, well under a page.
- **The ADR ships in the same commit as its code.** While it is still in the working diff
  it is editable — rewrite, renumber or delete it. Once committed, only its Status changes
  and a new ADR supersedes it. A record that has been fully superseded may later be deleted,
  but only in a commit that moves its argument into the successor and leaves no reference
  behind.
- Status is `Proposed`, `Accepted`, `Superseded by NNNN` or `Deprecated`.
- *Alternatives considered* carries the honest cost of the option that won, not only of
  the ones that lost.

## Index

Grouped by what the decision governs; the number is still the order it was taken in.

### Foundations and process

| # | Decision | Status |
|---|---|---|
| [0001](0001-workspace-layout.md) | Workspace split into nine crates | Accepted |
| [0002](0002-gui-framework-egui.md) | egui and eframe for the GUI | Accepted |
| [0003](0003-math-library.md) | glam and parry3d, f32 scalars | Accepted |
| [0004](0004-error-handling.md) | thiserror in libraries, anyhow in binaries | Accepted |
| [0037](0037-the-dev-profile-is-optimised.md) | The dev profile is optimised | Accepted |
| [0096](0096-the-product-is-encrust.md) | The product is Encrust, and its project file `.encrust` | Accepted |
| [0127](0127-orchestration-is-a-crate-not-a-binary.md) | Orchestration is a crate, not a binary | Accepted |
| [0129](0129-what-a-model-carries-lives-in-its-core.md) | What a model carries lives in the core that makes it | Accepted |
| [0159](0159-agpl-with-a-contributor-licence-agreement.md) | AGPL-3.0 with a contributor licence agreement | Accepted |
| [0160](0160-a-release-is-an-installer-per-platform.md) | A release is an installer per platform, built in CI | Superseded by 0162 (the trigger only) |
| [0161](0161-the-readers-are-fuzzed-outside-the-workspace.md) | The readers are fuzzed from a crate outside the workspace | Superseded by 0173 (the CI trigger only) |
| [0162](0162-release-please-cuts-the-release.md) | release-please cuts the release, and the build workflow is called by it | Accepted |
| [0172](0172-the-window-offers-a-signed-update-and-never-applies-one.md) | The window offers a signed update, and never applies one by itself | Accepted |
| [0173](0173-a-pull-request-runs-linux-and-main-runs-the-rest.md) | A pull request runs Linux, and main runs the rest | Accepted |
| [0174](0174-a-plate-is-a-crate-live-and-written-down.md) | A plate is a crate, live and written down | Accepted |
| [0176](0176-the-command-line-is-encrust-with-subcommands.md) | The command line is `encrust`, with subcommands, JSON and a clean Ctrl-C | Accepted |
| [0177](0177-the-cores-build-for-a-browser.md) | The cores build for a browser: wasm32, one thread, no clock, a cavity budget | Accepted |
| [0179](0179-a-plate-file-is-toml-read-by-the-command-line.md) | A plate file is TOML, read by the command line alone | Accepted |
| [0181](0181-the-window-runs-in-a-browser-on-workers.md) | The window runs in a browser, on workers sharing its memory | Accepted |
| [0183](0183-the-web-build-works-offline-and-the-command-line-completes-and-remembers.md) | The web build works offline, and the command line completes and remembers its flags | Accepted |

### Geometry

| # | Decision | Status |
|---|---|---|
| [0005](0005-mesh-repair.md) | Mesh repair in core-geometry; orientation only | Accepted |
| [0016](0016-raycasting-in-core-geometry.md) | Ray casting lives in core-geometry | Accepted |
| [0036](0036-rays-go-through-a-bounding-volume-hierarchy.md) | Every ray is cast through a bounding volume hierarchy | Accepted |
| [0053](0053-nearest-surface-queries-live-beside-the-ray-ones.md) | Nearest-surface queries live beside the ray ones, in `core-geometry` | Accepted |
| [0054](0054-inside-is-decided-by-the-generalised-winding-number.md) | Inside is decided by the generalised winding number | Accepted |
| [0055](0055-the-sign-is-pseudonormal-with-winding-behind-it.md) | The sign is the pseudonormal, with the winding number behind it | Accepted |
| [0075](0075-a-cut-is-its-own-operation.md) | A cut is its own operation, not part of hollowing | Accepted |
| [0086](0086-an-edit-is-taken-back-by-restoring-a-scene.md) | An edit is taken back by restoring a scene, not by inverting it | Accepted |
| [0089](0089-a-cut-is-exact-on-the-mesh.md) | A cut is exact on the mesh, capped with `earcutr` | Accepted |
| [0194](0194-a-hole-is-closed-only-when-it-is-asked-for.md) | A hole is closed only when it is asked for, by the triangulator the cut already uses | Accepted |
| [0195](0195-a-mended-model-is-sound-and-an-open-one-is-never-capped.md) | Repair drops the faces drawn twice, and an open model is never capped | Accepted |

### Mesh and texture input

| # | Decision | Status |
|---|---|---|
| [0038](0038-models-are-imported-on-a-worker-thread.md) | Models are imported on a worker thread | Accepted |
| [0111](0111-parse-obj-with-tobj.md) | OBJ is parsed with `tobj`, not by hand | Accepted |
| [0112](0112-resolve-the-3mf-graph-ourselves.md) | 3MF is parsed with `threemf2`, and its graph resolved here | Accepted |
| [0115](0115-uvs-are-a-sidecar-beside-the-mesh.md) | UVs are a sidecar beside the mesh, and the OBJ loader reads `map_Kd` | Accepted |
| [0121](0121-a-map-may-cover-part-of-a-mesh.md) | A UV map may cover part of a mesh; what it misses does not move | Accepted |
| [0123](0123-a-face-names-its-own-image.md) | A face names its own image, so a model is textured by material | Accepted |

### Slicing

| # | Decision | Status |
|---|---|---|
| [0006](0006-slicing-degenerate-cases.md) | Strict-sign vertex classification, stitching by mesh edge | Superseded by 0186 (an edge left twice only) |
| [0007](0007-slicing-performance-and-parallelism.md) | Z buckets for faces, rayon over layers | Accepted |
| [0008](0008-non-zero-winding-fill.md) | Non-zero winding fill, not even-odd | Accepted |
| [0066](0066-a-stack-is-sliced-a-window-at-a-time.md) | A stack is sliced a window of layers at a time | Accepted |
| [0071](0071-material-is-what-a-positive-winding-encloses.md) | Material is what a positive winding number encloses | Accepted; a cut's weight is amended by 0188 |
| [0091](0091-a-stack-is-planned-not-counted.md) | A stack is a plan of boundaries, not a count times a height | Accepted |
| [0114](0114-unite-a-layers-planes-by-brightness.md) | A layer is sampled at several planes, united by brightness on the runs | Accepted |
| [0134](0134-a-layer-starts-at-the-near-edge-of-the-plate.md) | Write the plate as it stands, and leave mirroring to the header | Accepted |
| [0187](0187-nothing-under-the-plate-is-cut.md) | Nothing under the plate is cut | Accepted |

### Rasterisation

| # | Decision | Status |
|---|---|---|
| [0009](0009-coverage-anti-aliasing.md) | Sub-scanline coverage anti-aliasing | Superseded by 0021 |
| [0010](0010-streaming-raster-stack.md) | Rasterise the stack in windows, bounded memory | Accepted |
| [0020](0020-layers-are-runs-not-pixels.md) | A rasterised layer leaves core-raster as runs, not pixels | Accepted |
| [0021](0021-exact-area-coverage.md) | Anti-alias by exact pixel area, not by sub-scanlines | Accepted |
| [0023](0023-preview-masks-are-downsampled.md) | Preview masks are max-pooled down to at most 2048 pixels | Accepted |
| [0039](0039-near-empty-and-near-full-pixels-snap.md) | Coverage within 2% of empty or full snaps to the panel's own black and white | Accepted |
| [0113](0113-write-only-the-grey-the-panel-cures.md) | A mask carries only the grey the panel will cure | Accepted |
| [0117](0117-blur-the-coverage-before-the-floor.md) | Blur is a box filter on the runs' coverage, before the floor | Accepted |
| [0139](0139-the-edge-grey-follows-a-file-that-prints.md) | Let the edge carry every grey | Accepted |
| [0145](0145-tolerance-moves-the-contours-not-the-mask.md) | Tolerance moves the contours, not the mask | Accepted |

### Exposure and the written stack

| # | Decision | Status |
|---|---|---|
| [0012](0012-streaming-sliced-file-writer.md) | Sliced-file writers take layers one at a time | Accepted |
| [0013](0013-exposure-settings-in-the-material-profile.md) | Exposure and motion settings live in MaterialProfile | Accepted |
| [0045](0045-sliced-files-are-written-to-a-seekable-sink.md) | Every sliced file is written to a seekable sink | Accepted |
| [0175](0175-a-sliced-file-leaves-through-a-sink.md) | A sliced file leaves through a sink, not a path | Accepted |
| [0067](0067-the-resin-volume-is-patched-in-at-finish.md) | The resin volume is patched into the header at `finish` | Accepted |
| [0090](0090-exposure-is-banded-by-height.md) | Exposure varies by bands of height, and the bottom block is out of reach | Accepted |
| [0128](0128-the-exposure-follows-the-layer-in-sight.md) | The exposure follows the layer in sight | Accepted |
| [0133](0133-a-written-height-is-a-whole-micron.md) | Round every height a file states to the micron | Accepted |
| [0143](0143-exposure-follows-the-working-curve.md) | Exposure follows the working curve, measured or assumed | Accepted |
| [0144](0144-compensation-belongs-to-the-resin-on-a-machine.md) | Compensation belongs to the resin on a machine | Accepted |

### Analysis

| # | Decision | Status |
|---|---|---|
| [0163](0163-the-stack-is-measured-from-its-runs.md) | The stack is measured from its runs, in a crate of its own | Accepted |
| [0164](0164-risks-are-read-off-the-stack-with-a-neck-carried-up.md) | Risks are read off the stack, each piece carrying the narrowest neck under it | Accepted |

### Supports

| # | Decision | Status |
|---|---|---|
| [0026](0026-supports-are-meshed-before-slicing.md) | Supports are meshed and merged into the model before slicing | Accepted |
| [0027](0027-support-profiles-live-in-printer-profiles.md) | SupportProfile lives in printer-profiles | Accepted |
| [0028](0028-supports-are-vertical-columns-anchored-in-model-space.md) | Contacts are kept in model space; columns are vertical | Accepted |
| [0029](0029-overhangs-are-found-on-the-slice-stack.md) | Islands and peninsulas are found on the slice stack | Accepted |
| [0031](0031-overhangs-are-measured-over-a-rise.md) | An overhang is measured over a rise, not from one layer to the next | Accepted |
| [0032](0032-layer-areas-are-read-as-grid-spans.md) | Layer areas are read as spans on a raster grid, not as polygons | Accepted |
| [0033](0033-placement-reads-layers-in-parallel-blocks.md) | Placement reads and examines layers in parallel blocks | Accepted |
| [0034](0034-overhangs-are-shaded-in-the-viewport.md) | Overhangs are shaded in the viewport shader, by face angle | Accepted |
| [0040](0040-nearby-tips-merge-into-shared-trunks.md) | Nearby tips merge into shared trunks, greedily, highest meeting point first | Accepted |
| [0041](0041-a-support-is-a-set-of-overlapping-tubes.md) | A support is meshed as overlapping closed tubes, not one welded solid | Accepted |
| [0042](0042-support-shapes-are-profile-data-swept-by-one-mesher.md) | Support shapes are data in the profile, swept by the one mesher | Accepted |
| [0043](0043-the-base-is-three-solids-beside-the-supports.md) | The raft, the skate and the braces are solids beside the supports | Accepted |
| [0044](0044-joints-are-balls-and-every-foot-is-bevelled.md) | A joint is covered by a ball, every foot is bevelled, braces cross | Accepted |
| [0072](0072-trapped-resin-is-found-on-the-stack.md) | Trapped resin is found on the slice stack, in `core-supports` | Accepted |
| [0077](0077-a-support-body-is-checked-as-a-beam.md) | A support body is checked as a beam, not as its axis | Accepted |
| [0078](0078-a-blocked-tip-leans-until-it-finds-room.md) | A blocked tip leans until it finds room | Accepted |
| [0079](0079-a-standing-support-holds-a-ball.md) | A standing support holds a ball, not a column of air | Accepted |
| [0080](0080-a-step-is-an-overhang-whatever-the-lean-says.md) | A step is an overhang, whatever the lean says | Accepted |
| [0081](0081-a-support-holds-only-the-part-it-stands-under.md) | A support holds only the part it stands under | Accepted |
| [0092](0092-a-painted-patch-is-a-set-of-faces.md) | A painted patch is a set of faces, filled on the plate's grid | Accepted |
| [0093](0093-a-blocker-is-a-keep-out-not-a-hint.md) | A support blocker is a keep-out, not a hint | Accepted |
| [0094](0094-a-supports-parameters-are-its-groups.md) | A support's parameters are its group's | Accepted |
| [0095](0095-a-support-the-hand-touched-is-frozen.md) | A support the hand has touched is frozen into the scene | Accepted |
| [0119](0119-support-profiles-join-the-catalogue.md) | Support profiles join the catalogue; a group tunes a copy | Accepted |
| [0124](0124-a-support-standing-on-the-part-ends-in-a-contact.md) | End a support that stands on the part in a contact, and let a profile refuse one | Accepted |
| [0125](0125-a-tip-leaves-the-surface-along-its-normal.md) | Leave the surface along its normal, and merge from the neck | Accepted |
| [0130](0130-a-pillar-flares-into-its-pad-at-forty-five-degrees.md) | Flare a pillar into its pad at forty-five degrees | Accepted |
| [0131](0131-the-shipped-presets-are-field-measurements.md) | Ship the field's measurements, and the two rules they broke | Accepted |
| [0132](0132-the-shipped-asset-is-the-preset.md) | The shipped TOML is the preset, and the constructor mirrors it | Accepted |

### Volume: hollowing, infill and relief

| # | Decision | Status |
|---|---|---|
| [0057](0057-a-field-is-band-tiles-and-solid-runs-on-a-global-lattice.md) | A field is band tiles and solid runs, on one global lattice | Accepted |
| [0058](0058-the-band-is-built-around-an-isosurface.md) | The band is built around an isosurface, not around the mesh | Accepted; the bottom-through field it prices is dropped by 0189 |
| [0059](0059-a-hollow-is-the-model-with-its-cavity-appended.md) | A hollow is the model with its cavity appended, not welded | Superseded by 0185 (the space the wall is measured in only); `BottomThrough` dropped by 0189 |
| [0060](0060-a-lattice-is-boxes-clipped-to-the-cavity.md) | A lattice is boxes clipped to the cavity, not a field | Accepted |
| [0063](0063-a-field-is-built-to-a-memory-budget.md) | A field is priced and built to a memory budget | Accepted; the clipped floor field it names is dropped by 0189 |
| [0064](0064-marching-cubes-merges-by-lattice-edge-in-layers.md) | Marching cubes merges by lattice edge, a layer at a time | Accepted |
| [0065](0065-a-fields-distances-are-quantised-to-its-band.md) | A field's distances are quantised to its band | Accepted |
| [0069](0069-the-band-walks-out-to-the-isosurface.md) | The band walks out to the isosurface rather than taking every tile near it | Accepted |
| [0076](0076-a-channel-is-a-pipe-through-the-part.md) | A channel is a pipe through the part, not a slot in it | Superseded by 0188 (waiting for a second run only) |
| [0082](0082-scatter-the-field-and-carry-it-coarse.md) | The field is scattered from the faces and carried across the wall coarse | Superseded by 0186 (where a far block's side comes from only) |
| [0083](0083-precision-is-capped-by-the-surface-it-pays-for.md) | Precision is capped by the surface a field pays for, not by the longest side | Accepted |
| [0084](0084-a-cavity-is-clustered-surface-nets.md) | A cavity is extracted as surface nets clustered onto the lattice | Superseded by 0186 (how folded copies are merged only) |
| [0085](0085-a-build-is-priced-by-its-larger-phase.md) | A build is priced by its larger phase, not by one constant a tile | Accepted |
| [0116](0116-a-texture-is-pressed-into-the-field.md) | A texture is pressed into the field, not onto the vertices | Accepted |
| [0122](0122-a-reliefs-depth-is-a-plate-millimetre.md) | A relief's depth is a plate millimetre, pressed after placement | Accepted |
| [0185](0185-a-wall-is-measured-on-the-plate.md) | A wall is measured on the plate, not in the model's own space | Accepted |
| [0186](0186-a-cavity-is-a-closed-surface-and-a-slice-follows-it-through-a-branch.md) | A cavity is a closed surface, and a slice follows it through a branch | Accepted |
| [0188](0188-a-cut-outweighs-what-it-lands-in-and-a-channel-stays-a-pipe.md) | A cut outweighs what it lands in, and a channel stays a pipe | Accepted |
| [0189](0189-resin-leaves-through-a-hole-and-a-lattice-never-closes-a-cell.md) | Resin leaves through a hole, and a lattice never closes a cell | Accepted; its marks are replaced by 0190 |
| [0190](0190-the-space-that-holds-resin-is-painted-and-the-model-is-seen-through.md) | Paint the space that holds resin, and see the model through | Accepted |

### The plate

| # | Decision | Status |
|---|---|---|
| [0087](0087-orientation-is-scored-on-the-peel.md) | An orientation is scored on the peel, and measured in two passes | Accepted |
| [0088](0088-the-plate-is-packed-as-a-bitmap.md) | The plate is packed as a bitmap into the corner, and the block centred | Accepted |
| [0098](0098-a-plate-is-a-number-on-the-object.md) | A plate is a number on the object, not a scene of its own | Accepted |
| [0101](0101-a-tool-works-on-the-selection-or-the-plate.md) | A tool works on what is picked, and on the whole plate only when nothing is | Accepted |
| [0178](0178-a-project-names-its-cavities-and-the-engine-opens-it.md) | A project names its cavities, and the engine opens it into a plate | Superseded by 0191 |
| [0191](0191-a-project-holds-the-plate-as-it-stands.md) | A project holds the plate as it stands, and opening it builds nothing | Accepted |

### Sliced-file formats

| # | Decision | Status |
|---|---|---|
| [0046](0046-ctb-layer-data-is-written-in-the-clear.md) | `.ctb` layer data is written unencrypted | Accepted |
| [0047](0047-the-output-extension-picks-the-format.md) | The output extension picks the sliced-file format, in each binary | Accepted |
| [0048](0048-thumbnails-are-rendered-in-software.md) | A sliced file's thumbnail is rendered in software, in `core-thumbnail` | Accepted |
| [0146](0146-the-chitu-family-is-one-crate-and-cbddlp-carries-grey-in-eight-passes.md) | Keep the Chitu family in one crate, and buy `.cbddlp` grey with eight passes | Accepted |
| [0147](0147-one-crate-writes-every-anycubic-extension-at-version-one.md) | Write every Anycubic extension from one crate, at version 1 | Accepted |
| [0148](0148-the-sl1-settings-go-in-last-and-a-varying-stack-is-refused.md) | Write the `.sl1` settings last, and refuse a stack it cannot carry | Accepted |
| [0149](0149-a-reader-is-a-trait-and-a-container-names-itself.md) | Make a reader a trait, and let a container name itself | Accepted |
| [0150](0150-an-open-file-owns-the-source-it-reads-from.md) | Let an open file own the source it reads from | Accepted |
| [0167](0167-the-gcode-zip-is-its-own-crate-and-the-png-codec-moves-down.md) | Give the gcode zip its own crate, and move the PNG codec down into `core-format` | Accepted |
| [0168](0168-one-creality-crate-holds-both-revisions-and-the-chitu-codecs-move-down.md) | Write both `.cxdlp` revisions from one crate, and move the Chitu codecs down | Accepted |
| [0169](0169-the-vector-container-traces-its-polygons-out-of-the-mask.md) | Trace the `.svgx` polygons out of the mask rather than carry contours to the writer | Accepted |
| [0170](0170-the-cws-is-its-own-crate-and-only-the-plain-variant-is-written.md) | Write the `.cws` from its own crate, and only the plain variant | Accepted |
| [0171](0171-a-count-read-from-a-file-is-checked-against-the-file.md) | Check every count a file states against the file before reserving for it | Accepted |
| [0180](0180-a-sliced-file-is-converted-in-the-write-stage.md) | A sliced file is converted in the write stage, never resampled | Accepted |

### Printer and material profiles

| # | Decision | Status |
|---|---|---|
| [0049](0049-profiles-ship-in-the-binary-with-a-user-directory-over-them.md) | Profiles ship inside the binary, with a user directory over them by id | Accepted |
| [0050](0050-one-resin-catalogue-retuned-per-printer.md) | One resin catalogue, retuned per printer by a `[printers.<id>]` table | Accepted |
| [0051](0051-a-printer-profile-names-its-file-format.md) | A printer profile names the sliced-file format its firmware reads | Accepted |
| [0052](0052-profiles-are-edited-in-the-window.md) | Profiles are edited in a Settings screen and saved into the user's directory | Accepted |
| [0120](0120-a-resin-is-tuned-per-printer.md) | A printer's resins are its own presets, edited in place | Accepted |
| [0140](0140-a-profile-carries-the-machine-name-its-firmware-matches.md) | A profile carries the machine name its firmware matches | Accepted |
| [0141](0141-advance-mode-follows-the-firmware-flag.md) | Advance mode follows the machine's firmware flag | Accepted |
| [0142](0142-a-tilting-vat-reads-the-header-alone.md) | A tilting vat reads the header alone | Accepted |
| [0156](0156-a-printer-profile-names-its-format-and-its-machine.md) | A printer profile names its format, and is bound to its machine | Accepted |
| [0158](0158-the-shipped-catalogue-is-a-library-not-a-list-of-machines-you-own.md) | The shipped catalogue is a library, not the machines you own | Accepted |
| [0165](0165-the-catalogue-is-generated-and-only-what-we-can-write-ships.md) | The catalogue is generated, and only machines we can write a file for ship | Accepted |
| [0166](0166-a-machine-states-which-photon-workshop-revision-it-reads.md) | A machine states which Photon Workshop revision it reads | Accepted |
| [0196](0196-a-resin-is-measured-not-shipped.md) | A resin is measured, not shipped, and a copy says where it came from | Superseded by 0197 (what a resin taken off a printer does only) |
| [0197](0197-a-resin-off-a-printer-waits-in-the-pool.md) | A resin taken off a printer waits in the pool | Accepted |

### Printers over the network

| # | Decision | Status |
|---|---|---|
| [0136](0136-a-network-protocol-is-a-crate-of-its-own.md) | A network protocol is a crate of its own | Accepted |
| [0137](0137-the-network-client-is-synchronous.md) | The network client is synchronous | Accepted |
| [0152](0152-the-file-goes-up-as-one-put-and-nothing-else.md) | The file goes up as one PUT and nothing else | Accepted |
| [0153](0153-a-prusa-machine-is-set-up-not-discovered.md) | A Prusa machine is set up, not discovered | Superseded by 0182 (where the enum lives only) |
| [0154](0154-sdcp-version-one-is-the-same-crate-with-a-second-transport.md) | SDCP version 1 is the same crate with a second transport | Accepted |
| [0155](0155-the-broker-and-the-file-server-are-written-here.md) | The broker and the file server are written here | Accepted |
| [0182](0182-sending-is-a-crate-both-front-ends-share-and-a-browser-downloads.md) | Sending is a crate both front ends share, and a browser downloads | Accepted |

### The window and the command line

| # | Decision | Status |
|---|---|---|
| [0011](0011-png-stack-in-the-cli.md) | The PNG stack lives in encrust-cli, written with png | Accepted |
| [0014](0014-viewport-in-the-egui-render-pass.md) | The viewport paints into egui's own render pass | Superseded by 0184 (the scene's pass only) |
| [0015](0015-scene-owns-meshes-behind-arc.md) | The window owns the scene; meshes are shared behind Arc | Accepted |
| [0017](0017-transform-gizmo-crate.md) | transform-gizmo-egui for the transform handles | Accepted |
| [0018](0018-background-slicing-job.md) | Slicing runs on a worker thread and reports over a channel | Accepted |
| [0019](0019-slicing-job-thread-budget.md) | A background job runs on its own pool, one thread short of the machine | Accepted |
| [0024](0024-design-tokens-and-bundled-typeface.md) | One token module paints the window; the typeface is compiled in | Accepted |
| [0025](0025-fixed-window-layout.md) | Fixed panels instead of egui_dock | Accepted |
| [0061](0061-the-section-slider-is-one-rail-in-both-modes.md) | The section slider is one rail, in both modes | Accepted |
| [0062](0062-cap-the-section-cut-with-the-stencil-plane.md) | The section cut is capped with the stencil plane | Superseded by 0184 (where the stencil comes from only) |
| [0068](0068-the-window-holds-no-stack.md) | The window holds no stack: it cuts the window it is showing | Accepted |
| [0070](0070-a-mesh-is-drawn-in-buffer-sized-pieces.md) | A mesh is drawn in pieces the card will take | Accepted; 0190 breaks a piece at a drawn range too |
| [0073](0073-the-viewport-subtracts-a-drain-per-fragment.md) | The viewport subtracts a drain per fragment | Superseded by 0188 (a channel's wall only) |
| [0074](0074-the-section-cap-counts-material-not-crossings.md) | The section cap counts material, not crossings | Accepted |
| [0184](0184-the-viewport-draws-into-its-own-target.md) | The viewport draws into its own target | Accepted |
| [0097](0097-a-project-is-a-zip-of-a-manifest-and-the-meshes.md) | A project is a zip of a JSON manifest and the meshes | Superseded by 0191 |
| [0099](0099-unsaved-work-is-a-digest-asked-for-at-the-door.md) | Unsaved work is a digest of the manifest, asked for only at the door | Accepted |
| [0100](0100-a-batch-is-the-single-run-once-per-model.md) | A batch is the single run, once per model, in the same binary | Accepted |
| [0102](0102-the-plate-is-a-panel-not-a-card.md) | The plate is a panel, not a card over the viewport | Accepted |
| [0103](0103-preview-splits-the-stage.md) | Preview splits the stage between the model and the mask | Accepted |
| [0104](0104-the-title-strip-is-the-title-bar.md) | The title strip is the window's title bar | Accepted |
| [0105](0105-a-panel-belongs-to-its-tool.md) | A panel belongs to its tool | Superseded by 0118 (the transform switch only) |
| [0106](0106-a-shortcut-lives-in-the-table.md) | Every key the window answers lives in one table | Accepted |
| [0107](0107-take-a-colour-token-not-floats.md) | A colour crosses into the renderer as a token, not as floats | Accepted |
| [0108](0108-the-machine-is-drawn-from-the-build-volume.md) | The build platform is drawn from the build volume, not loaded from a model | Accepted |
| [0109](0109-the-preview-is-read-against-the-plate.md) | The preview is read against the plate, not against the panel | Accepted |
| [0110](0110-the-gizmo-stands-on-the-centre-of-mass.md) | The gizmo stands on the model's centre of mass | Accepted |
| [0118](0118-one-placing-panel-and-a-standing-gizmo.md) | One placing panel, and a gizmo that is always out | Accepted |
| [0135](0135-the-macos-icon-is-squircled-at-source.md) | The macOS icon is squircled at source, not only in the bundle | Accepted |
| [0138](0138-the-destination-lives-on-the-slice-button.md) | The destination lives on the Slice button | Superseded by 0157 |
| [0151](0151-the-preview-shows-a-source-not-a-plate.md) | Let the preview show a source, which is either a plate or a file | Accepted |
| [0157](0157-the-slice-row-carries-two-actions.md) | The Slice row carries two actions, not a destination | Accepted |
| [0192](0192-a-tool-value-is-remembered-and-taken-back-on-its-own-entry.md) | A tool value is remembered between runs and taken back on its own entry | Accepted |
| [0193](0193-the-window-reads-its-own-input-before-egui-does.md) | The window reads its own input before egui does | Accepted |

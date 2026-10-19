use std::collections::HashMap;
use std::io::Read as _;
use std::sync::{Arc, Weak};

use anyhow::{Context as _, Result, bail};
use core_geometry::{Heightmap, Mapping, Mat3, Mesh, Scalar, UvMap, Vec2, Vec3};
use core_volume::{AngleMask, ReliefBasis, ReliefSettings, Tiling, prepared};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::files::{self, Handed, Wanted};
use crate::i18n::tr;
use crate::job::{
    Pressed, ReliefJob, ReliefOutcome, ReliefRequest, ReliefSource, relief_tasks, spawn_relief,
};
use crate::scene::{Mapped, ObjectId, Scene};
use crate::status::Status;

mod library;

pub use library::BUILTINS;

/// The texture each model is drawn with while the Relief tool is open.
pub type Textures = HashMap<ObjectId, Arc<Mapped>>;

/// What a model's drawn texture was made for, so it is made again only when one changes.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LaidFor {
    mesh: usize,
    /// The chosen picture's heights, or the model's own texture.
    source: usize,
    sizing: Mat3,
    tiling: Tiling,
    look: Look,
}

/// What of the panel changes the picture itself, rather than where it is pressed.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Look {
    invert: bool,
    smoothing_px: Scalar,
}

/// The widest a picture is read at, pixels. Past this a tile of a few millimetres has
/// more pixels than the lattice can show, and the samples only cost memory.
const MAX_PICTURE_PX: u32 = 1024;

/// A greyscale picture chosen to press in as tiles, in place of a model's own texture.
#[derive(Debug, Clone)]
pub struct Picture {
    pub name: String,
    pub heights: Arc<Heightmap>,
    /// The size of the file, pixels, before it was read down to `MAX_PICTURE_PX`.
    pub size_px: [u32; 2],
    /// Which of [`BUILTINS`] it is, for one that came with the window.
    pub builtin: Option<usize>,
}

/// What of the Relief tool the window remembers between runs and no undo takes back, like a
/// view: the mode it is in and the built-in texture chosen, by its file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReliefChoice {
    pub mode: ReliefMode,
    pub texture: Option<String>,
}

/// How much memory the bases kept between presses may hold, bytes. A browser's memory
/// only ever grows and is shared with every other tab, so the page keeps far less.
#[cfg(target_arch = "wasm32")]
const KEPT_BYTES: usize = 192 << 20;
#[cfg(not(target_arch = "wasm32"))]
const KEPT_BYTES: usize = 768 << 20;

/// The basis one model was last pressed on, kept while the model it was built over is
/// still the one on the plate — which, once pressed, it is again only after an undo.
#[derive(Debug)]
struct Kept {
    id: ObjectId,
    mesh: Weak<Mesh>,
    /// The size and turn the mesh was pressed at, which a mirror changes and its box may not.
    sizing: Mat3,
    basis: Arc<ReliefBasis>,
}

/// What the Relief tool presses with. It is the window's own record: the project manifest
/// has no entry for this tool (ADR 0191).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReliefState {
    /// How far the surface moves where the texture is white, millimetres. Negative sinks
    /// the relief in rather than raising it.
    pub amplitude_mm: Scalar,
    pub precision: Scalar,
    #[serde(default)]
    pub invert: bool,
    #[serde(default)]
    pub symmetric: bool,
    /// How far the texture is blurred before it is pressed, pixels of the texture.
    #[serde(default)]
    pub smoothing_px: Scalar,
    /// Faces within this many degrees of facing the plate are left as they are.
    #[serde(default)]
    pub bottom_deg: Scalar,
    /// Faces within this many degrees of facing straight up are left as they are.
    #[serde(default)]
    pub top_deg: Scalar,
}

impl Default for ReliefState {
    fn default() -> Self {
        let asked = ReliefSettings::default();
        Self {
            amplitude_mm: asked.amplitude_mm,
            precision: asked.precision,
            invert: asked.invert,
            symmetric: asked.symmetric,
            smoothing_px: asked.smoothing_px,
            bottom_deg: asked.mask.bottom_deg,
            top_deg: asked.mask.top_deg,
        }
    }
}

/// Which of the Relief tool's three jobs the panel and a click are for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReliefMode {
    /// A texture or a picture pressed into the surface.
    #[default]
    Texture,
    /// Text written where a model is clicked.
    Text,
    /// The picked models joined, subtracted or intersected.
    Combine,
}

/// What the Relief tool is set to, and the run it has going.
///
/// A texture is pressed into the model as geometry rather than written as exposure: grey
/// controls cure depth, so it could only texture a face pointing up or down; see
/// `docs/decisions/0116-a-texture-is-pressed-into-the-field.md`.
#[derive(Debug, Default)]
pub struct ReliefTool {
    pub mode: ReliefMode,
    pub state: ReliefState,
    pub job: Option<ReliefJob>,
    /// The picture pressed instead of the models' own textures, while one is chosen. The
    /// window's own, like the tiling: a project does not write it down.
    pub picture: Option<Picture>,
    pub tiling: Tiling,
    /// The picture laid over each model it would be pressed into, for the view to draw.
    laid: HashMap<ObjectId, (LaidFor, Arc<Mapped>)>,
    /// What the last run stood on, so that trying another picture after an undo skips
    /// the search that is most of a press.
    kept: Vec<Kept>,
}

impl ReliefTool {
    /// What the panel is set to. The mask's up is the plate's; each run turns it into
    /// the model's own.
    pub fn settings(&self) -> ReliefSettings {
        let state = &self.state;
        ReliefSettings {
            amplitude_mm: state.amplitude_mm,
            precision: state.precision,
            invert: state.invert,
            symmetric: state.symmetric,
            smoothing_px: state.smoothing_px,
            mask: AngleMask {
                bottom_deg: state.bottom_deg,
                top_deg: state.top_deg,
                ..AngleMask::default()
            },
            ..ReliefSettings::default()
        }
    }

    fn look(&self) -> Look {
        Look {
            invert: self.state.invert,
            smoothing_px: self.state.smoothing_px,
        }
    }

    /// Why the Press button is greyed out, or `None` when it is not.
    pub fn blocker(&self, scene: &Scene) -> Option<String> {
        if scene.target_count() == 0 {
            return Some(tr!("relief-nothing-visible"));
        }
        if self.picture.is_none() && relief_tasks(scene, false).is_empty() {
            return Some(tr!("relief-no-textures"));
        }
        None
    }

    /// Starts a run over everything visible on the plate that carries a texture.
    pub fn start(&mut self, scene: &Scene) -> Result<()> {
        let mut tasks = relief_tasks(scene, self.picture.is_some());
        if tasks.is_empty() {
            bail!(tr!("relief-nothing-to-press"));
        }
        for task in &mut tasks {
            task.basis = self
                .kept
                .iter()
                .find(|kept| {
                    kept.id == task.taken.id
                        && kept.mesh.as_ptr() == Arc::as_ptr(&task.mesh)
                        && kept.sizing == task.sizing.linear()
                })
                .map(|kept| Arc::clone(&kept.basis));
        }
        // What this run does not take is dropped before it starts rather than after, so
        // it is never resident beside the bases the run builds.
        self.kept.clear();
        let source = match &self.picture {
            Some(picture) => ReliefSource::Tiled {
                image: Arc::clone(&picture.heights),
                tiling: self.tiling,
            },
            None => ReliefSource::Mapped,
        };

        self.job = Some(spawn_relief(ReliefRequest {
            tasks,
            settings: self.settings(),
            source,
        }));
        Ok(())
    }

    /// Drains a running job into the scene and the stage notice. Returns whether one is
    /// still going, which is what tells the window to keep repainting.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let Some(job) = self.job.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return true;
        };

        self.job = None;
        *status = match outcome {
            ReliefOutcome::Pressed(models) => self.land(scene, models),
            ReliefOutcome::Failed(message) => Status::Error(message),
        };
        false
    }

    /// Puts each pressed mesh on the model it was pressed into, unless that model has been
    /// reshaped, turned or resized since, and keeps the bases there is room for.
    fn land(&mut self, scene: &mut Scene, models: Vec<Pressed>) -> Status {
        let count = models.len();
        let coarsened = models.iter().filter(|model| model.coarsened).count();
        let mut changed = None;
        let mut room = KEPT_BYTES;
        for model in models {
            let Some(object) = scene
                .get_mut(model.taken.id)
                .filter(|object| model.taken.is_shaped_alike_to(object))
            else {
                changed.get_or_insert(model.taken.name);
                continue;
            };
            if model.basis.bytes() <= room {
                room -= model.basis.bytes();
                self.kept.push(Kept {
                    id: object.id,
                    mesh: Arc::downgrade(&object.mesh),
                    sizing: object.transform.sizing().linear(),
                    basis: model.basis,
                });
            }
            // The relief was pressed at printed size, so the scale and the stretch are in
            // the mesh now and must not be applied twice.
            object.reshape(model.shaped);
            object.transform.scale = Vec3::ONE;
            object.transform.stretch = Mat3::IDENTITY;
        }
        if let Some(name) = changed {
            return Status::Error(tr!("job-model-changed", name = name));
        }
        let depth = format!("{:.2}", self.state.amplitude_mm);
        match coarsened {
            0 => Status::Info(tr!("relief-done", depth = depth, count = count)),
            _ => Status::Info(tr!(
                "relief-done-coarsened",
                depth = depth,
                count = count,
                coarsened = coarsened
            )),
        }
    }

    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }

    /// The mode and built-in texture to remember.
    pub fn choice(&self) -> ReliefChoice {
        let builtin = self.picture.as_ref().and_then(|picture| picture.builtin);
        ReliefChoice {
            mode: self.mode,
            texture: builtin
                .and_then(|index| BUILTINS.get(index))
                .map(|b| b.file.to_owned()),
        }
    }

    /// Stands the tool in the mode and on the texture remembered; a texture no longer
    /// among the built-in ones is left unchosen.
    pub fn restore(&mut self, choice: &ReliefChoice) -> Result<()> {
        self.mode = choice.mode;
        let Some(index) = choice.texture.as_ref().and_then(|file| {
            BUILTINS
                .iter()
                .position(|builtin| builtin.file == file.as_str())
        }) else {
            return Ok(());
        };
        self.picture = Some(builtin_picture(index)?);
        Ok(())
    }

    /// Gives up on the press going: what it would have made is thrown away when it ends.
    pub fn cancel(&mut self, status: &mut Status) {
        if self.job.take().is_some() {
            *status = Status::Info(tr!("relief-cancelled"));
        }
    }

    /// Lets go of the bases kept for another press, which the window does once the tool
    /// is put down: they can be hundreds of megabytes.
    pub fn forget_kept(&mut self) {
        self.kept.clear();
    }

    /// The texture each model on the plate is drawn with, read the way pressing reads it:
    /// the chosen picture over the models it would be pressed into, else the texture each
    /// model carries.
    pub fn textures(&mut self, scene: &Scene) -> Textures {
        let look = self.look();
        let settings = self.settings();
        let mut laid = HashMap::new();
        match &self.picture {
            Some(picture) => {
                // Read once for every model it is laid over, and only when one is laid anew.
                let mut read: Option<Picture> = None;
                for object in scene.targets() {
                    let key = LaidFor {
                        mesh: Arc::as_ptr(&object.mesh) as usize,
                        source: Arc::as_ptr(&picture.heights) as usize,
                        sizing: object.transform.sizing().linear(),
                        tiling: self.tiling,
                        look,
                    };
                    let mapped = relaid(&mut self.laid, object.id, key, || {
                        let read = read.get_or_insert_with(|| Picture {
                            heights: Arc::new(prepared(&picture.heights, &settings).into_owned()),
                            ..picture.clone()
                        });
                        Arc::new(lay(read, &object.mesh, key.sizing, self.tiling))
                    });
                    laid.insert(object.id, (key, mapped));
                }
            }
            None => {
                for object in scene.printable(scene.active_plate()) {
                    let Some(own) = &object.mapped else {
                        continue;
                    };
                    let key = LaidFor {
                        mesh: Arc::as_ptr(&object.mesh) as usize,
                        source: Arc::as_ptr(own) as usize,
                        sizing: Mat3::IDENTITY,
                        tiling: Tiling::default(),
                        look,
                    };
                    let mapped = relaid(&mut self.laid, object.id, key, || {
                        read_as_pressed(own, &settings)
                    });
                    laid.insert(object.id, (key, mapped));
                }
            }
        }
        self.laid = laid;
        self.laid
            .iter()
            .map(|(id, (_, mapped))| (*id, Arc::clone(mapped)))
            .collect()
    }
}

/// What was laid over `id` before where it was laid for `key`, or what `make` lays anew.
fn relaid(
    was: &mut HashMap<ObjectId, (LaidFor, Arc<Mapped>)>,
    id: ObjectId,
    key: LaidFor,
    make: impl FnOnce() -> Arc<Mapped>,
) -> Arc<Mapped> {
    match was.remove(&id) {
        Some((laid_for, mapped)) if laid_for == key => mapped,
        _ => make(),
    }
}

/// A model's own texture as pressing with `settings` reads it, shared as it is where that
/// changes nothing.
fn read_as_pressed(own: &Arc<Mapped>, settings: &ReliefSettings) -> Arc<Mapped> {
    if settings.smoothing_px <= 0.0 && !settings.invert {
        return Arc::clone(own);
    }
    Arc::new(Mapped {
        names: own.names.clone(),
        uvs: own.uvs.clone(),
        heights: own
            .heights
            .iter()
            .map(|image| prepared(image, settings).into_owned())
            .collect(),
    })
}

/// `picture` laid over `mesh` at printed size the way pressing it lays it, each face read
/// from the one of the three planes it faces most, where pressing blends them.
fn lay(picture: &Picture, mesh: &Mesh, sizing: Mat3, tiling: Tiling) -> Mapped {
    let (sin, cos) = tiling.angle_deg.to_radians().sin_cos();
    let on_plane = |a: Scalar, b: Scalar| Vec2::new(a * cos - b * sin, a * sin + b * cos);
    let faces = mesh
        .faces
        .par_iter()
        .map(|face| {
            let corners = face.map(|index| mesh.vertices.get(index as usize).map(|v| sizing * *v));
            let [Some(a), Some(b), Some(c)] = corners else {
                return None;
            };
            let facing = (b - a).cross(c - a).abs();
            let project = |p: Vec3| {
                let flat = if facing.x >= facing.y && facing.x >= facing.z {
                    on_plane(p.y, p.z)
                } else if facing.y >= facing.z {
                    on_plane(p.x, p.z)
                } else {
                    on_plane(p.x, p.y)
                };
                flat / tiling.tile_mm
            };
            Some(Mapping::sole([a, b, c].map(project)))
        })
        .collect();
    Mapped {
        names: vec![picture.name.clone()],
        uvs: UvMap::new(faces),
        heights: vec![(*picture.heights).clone()],
    }
}

/// Asks for a picture and takes it as what the Relief tool presses in. In a browser the
/// answer comes on a later frame, through [`load_picture`].
pub fn open_picture(tool: &mut ReliefTool, status: &mut Status) {
    if let Some(file) = files::pick(Wanted::ReliefPicture) {
        load_picture(tool, status, &file);
    }
}

/// Reads `file` as a greyscale picture for the Relief tool.
pub fn load_picture(tool: &mut ReliefTool, status: &mut Status, file: &Handed) {
    let path = file.path();
    let read = read_picture(file)
        .with_context(|| tr!("error-cannot-read", path = path.display().to_string()));
    let picked = tr!("relief-picked-picture", name = picture_name(file));
    if let Some(picture) = status.report(&picked, read) {
        tool.picture = Some(picture);
    }
}

fn picture_name(file: &Handed) -> String {
    file.path()
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

fn read_picture(file: &Handed) -> Result<Picture> {
    let mut bytes = Vec::new();
    file.reader()?.read_to_end(&mut bytes)?;
    decoded(picture_name(file), &bytes)
}

/// Takes one of the textures that come with the window as what the Relief tool presses in.
pub fn choose_builtin(tool: &mut ReliefTool, status: &mut Status, index: usize) {
    let Some(builtin) = BUILTINS.get(index) else {
        return;
    };
    let picked = tr!("relief-picked-picture", name = (builtin.name)());
    if let Some(picture) = status.report(&picked, builtin_picture(index)) {
        tool.picture = Some(picture);
    }
}

/// One of [`BUILTINS`] read as the picture to press, or an error past the end of them.
fn builtin_picture(index: usize) -> Result<Picture> {
    let builtin = BUILTINS
        .get(index)
        .with_context(|| format!("no built-in texture {index}"))?;
    decoded((builtin.name)(), builtin.bytes).map(|picture| Picture {
        builtin: Some(index),
        ..picture
    })
}

/// `bytes` of any picture format read as grey, no wider than `MAX_PICTURE_PX`.
fn decoded(name: String, bytes: &[u8]) -> Result<Picture> {
    let image = image::load_from_memory(bytes)?;
    let size_px = [image.width(), image.height()];
    let image = if size_px[0].max(size_px[1]) > MAX_PICTURE_PX {
        image.thumbnail(MAX_PICTURE_PX, MAX_PICTURE_PX)
    } else {
        image
    };
    let grey = image.to_luma32f();
    let (width, height) = (grey.width() as usize, grey.height() as usize);
    let heights =
        Heightmap::new(width, height, grey.into_raw()).context(tr!("relief-no-pixels"))?;
    Ok(Picture {
        name,
        heights: Arc::new(heights),
        size_px,
        builtin: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_with_a_tetrahedron() -> Scene {
        use core_geometry::{Mesh, Orientation, Transform, diagnose};
        let mesh = Mesh::new(
            vec![
                Vec3::ZERO,
                Vec3::new(20.0, 0.0, 0.0),
                Vec3::new(0.0, 20.0, 0.0),
                Vec3::new(0.0, 0.0, 20.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        );
        let mut scene = Scene::default();
        scene.insert(crate::scene::Imported::new(
            "tetrahedron".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            crate::scene::ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));
        scene
    }

    #[test]
    fn a_picture_is_read_as_its_grey_and_kept_by_name() {
        let mut png = Vec::new();
        let white = image::GrayImage::from_pixel(2, 3, image::Luma([255]));
        white
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("a png writes into memory");
        let file = Handed::bytes("stone.png", Arc::from(png));

        let mut tool = ReliefTool::default();
        load_picture(&mut tool, &mut Status::default(), &file);
        let picture = tool.picture.expect("a png reads");
        assert_eq!(picture.name, "stone.png");
        assert_eq!(picture.size_px, [2, 3]);
        assert!((picture.heights.sample(core_geometry::Vec2::splat(0.5)) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_builtin_texture_is_chosen_by_its_place_in_the_list() {
        let mut tool = ReliefTool::default();
        choose_builtin(&mut tool, &mut Status::default(), 2);
        let picture = tool.picture.expect("a shipped texture reads");
        assert_eq!(picture.builtin, Some(2));
        assert_eq!(picture.name, (BUILTINS[2].name)());
    }

    #[test]
    fn a_file_that_is_no_picture_is_said_so() {
        let mut tool = ReliefTool::default();
        let mut status = Status::default();
        let file = Handed::bytes("notes.png", Arc::from(&b"not a picture"[..]));
        load_picture(&mut tool, &mut status, &file);
        assert!(tool.picture.is_none());
        assert!(status.is_error());
    }

    #[test]
    fn a_picture_presses_into_a_model_with_no_texture() {
        let mut tool = ReliefTool::default();
        let scene = scene_with_a_tetrahedron();
        assert!(
            tool.blocker(&scene).is_some(),
            "an untextured cube has nothing to press"
        );
        tool.picture = Some(Picture {
            name: "flat.png".to_owned(),
            heights: Arc::new(Heightmap::new(1, 1, vec![1.0]).expect("one pixel")),
            size_px: [1, 1],
            builtin: None,
        });
        assert_eq!(tool.blocker(&scene), None);
    }

    #[test]
    fn an_empty_plate_says_there_is_nothing_to_press_into() {
        let mut tool = ReliefTool::default();
        let scene = Scene::default();
        assert_eq!(
            tool.blocker(&scene).as_deref(),
            Some("Nothing visible on the plate to press a texture into.")
        );
        assert!(tool.start(&scene).is_err());
    }

    #[test]
    fn a_chosen_picture_is_laid_over_the_models_it_would_press_and_kept_while_nothing_changes() {
        let scene = scene_with_a_tetrahedron();
        let mut tool = ReliefTool::default();
        assert!(
            tool.textures(&scene).is_empty(),
            "the tetrahedron carries no texture of its own"
        );

        tool.picture = Some(Picture {
            name: "stone.png".to_owned(),
            heights: Arc::new(Heightmap::new(1, 1, vec![1.0]).expect("one pixel")),
            size_px: [1, 1],
            builtin: None,
        });
        let first = tool.textures(&scene);
        let laid = first
            .values()
            .next()
            .expect("the picture is laid over the model");
        assert_eq!(
            laid.uvs.mapped(),
            4,
            "every face of the tetrahedron is mapped"
        );
        let again = tool.textures(&scene);
        assert!(
            Arc::ptr_eq(laid, again.values().next().expect("still laid")),
            "nothing changed, so the view keeps what it uploaded"
        );

        tool.tiling.tile_mm *= 2.0;
        let retiled = tool.textures(&scene);
        assert!(!Arc::ptr_eq(
            laid,
            retiled.values().next().expect("still laid")
        ));
    }

    #[test]
    fn a_face_is_laid_from_the_plane_it_faces_at_the_tile_size() {
        let picture = Picture {
            name: "stone.png".to_owned(),
            heights: Arc::new(Heightmap::new(1, 1, vec![1.0]).expect("one pixel")),
            size_px: [1, 1],
            builtin: None,
        };
        // A face in the plane z = 0, so it is read from above, 10 mm on a side.
        let mesh = Mesh::new(
            vec![
                Vec3::ZERO,
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(0.0, 10.0, 0.0),
            ],
            vec![[0, 1, 2]],
        );
        let tiling = Tiling {
            tile_mm: 5.0,
            angle_deg: 0.0,
        };
        let laid = lay(&picture, &mesh, Mat3::IDENTITY, tiling);
        let corners = laid.uvs.of_face(0).expect("the face is mapped");
        assert_eq!(
            corners.corners,
            [Vec2::ZERO, Vec2::new(2.0, 0.0), Vec2::new(0.0, 2.0)],
            "10 mm over 5 mm tiles is two tiles"
        );
    }

    #[test]
    fn the_settings_are_what_the_panel_is_set_to() {
        let tool = ReliefTool {
            state: ReliefState {
                amplitude_mm: -0.3,
                precision: 0.8,
                ..ReliefState::default()
            },
            ..ReliefTool::default()
        };
        let settings = tool.settings();
        assert!((settings.amplitude_mm + 0.3).abs() < 1e-6);
        assert!((settings.precision - 0.8).abs() < 1e-6);
    }

    #[test]
    fn an_inverted_picture_is_drawn_the_way_it_is_pressed() {
        let scene = scene_with_a_tetrahedron();
        let mut tool = ReliefTool {
            picture: Some(Picture {
                name: "white.png".to_owned(),
                heights: Arc::new(Heightmap::new(1, 1, vec![1.0]).expect("one pixel")),
                size_px: [1, 1],
                builtin: None,
            }),
            state: ReliefState {
                invert: true,
                ..ReliefState::default()
            },
            ..ReliefTool::default()
        };
        let drawn = tool.textures(&scene);
        let laid = drawn
            .values()
            .next()
            .expect("the picture is laid over the model");
        assert!(
            laid.heights[0].sample(Vec2::splat(0.5)).abs() < 1e-6,
            "white turned over is drawn black, as it would press"
        );
    }

    #[test]
    fn a_turned_model_holds_the_faces_that_face_the_plate_once_turned() {
        let mut scene = scene_with_a_tetrahedron();
        let id = scene.objects()[0].id;
        // A quarter turn about X stands the model's +Y up on the plate.
        scene
            .get_mut(id)
            .expect("the tetrahedron is in the scene")
            .transform
            .rotation = core_geometry::Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let tasks = relief_tasks(&scene, true);
        let up = tasks[0].up;
        assert!(
            (up - Vec3::Y).length() < 1e-5,
            "got {up}, expected the model's +Y"
        );
    }

    #[test]
    fn a_relief_pressed_into_a_model_resized_meanwhile_is_dropped() {
        let mut scene = scene_with_a_tetrahedron();
        let mut tool = ReliefTool {
            picture: Some(Picture {
                name: "flat.png".to_owned(),
                heights: Arc::new(Heightmap::new(1, 1, vec![1.0]).expect("one pixel")),
                size_px: [1, 1],
                builtin: None,
            }),
            ..ReliefTool::default()
        };
        tool.start(&scene)
            .expect("the picture presses into the tetrahedron");
        let object = &mut scene.objects_mut()[0];
        let before = Arc::clone(&object.mesh);
        object.transform.scale = Vec3::splat(2.0);
        let mut status = Status::default();
        while tool.poll(&mut scene, &mut status) {
            std::thread::yield_now();
        }
        assert_eq!(
            status,
            Status::Error(tr!("job-model-changed", name = "tetrahedron"))
        );
        let object = &scene.objects()[0];
        assert!(Arc::ptr_eq(&object.mesh, &before), "the mesh is left alone");
        assert_eq!(
            object.transform.scale,
            Vec3::splat(2.0),
            "and so is its size"
        );
    }

    #[test]
    fn the_mode_and_texture_chosen_come_back_in_the_next_run() {
        let mut tool = ReliefTool {
            mode: ReliefMode::Combine,
            ..ReliefTool::default()
        };
        choose_builtin(&mut tool, &mut Status::default(), 2);
        let text = serde_json::to_string(&tool.choice()).expect("the choice serialises");
        let read: ReliefChoice = serde_json::from_str(&text).expect("and reads back");
        let mut next = ReliefTool::default();
        next.restore(&read).expect("a built-in texture reads");
        assert_eq!(next.mode, ReliefMode::Combine);
        assert_eq!(next.picture.and_then(|picture| picture.builtin), Some(2));
    }
}

use std::collections::HashMap;
use std::io::Read as _;
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use core_geometry::{Heightmap, Mapping, Mat3, Mesh, Scalar, UvMap, Vec2, Vec3};
use core_volume::{ReliefSettings, Tiling};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::files::{self, Handed, Wanted};
use crate::job::{ReliefJob, ReliefOutcome, ReliefRequest, ReliefSource, relief_tasks};
use crate::scene::{Mapped, ObjectId, Scene};
use crate::status::Status;

/// The texture each model is drawn with while the Relief tool is open.
pub type Textures = HashMap<ObjectId, Arc<Mapped>>;

/// What a model's laid picture was laid for, so it is laid again only when one changes.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LaidFor {
    mesh: usize,
    picture: usize,
    sizing: Mat3,
    tiling: Tiling,
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
}

/// What the Relief tool presses with. It is the window's own record: the project manifest
/// has no entry for this tool (ADR 0191).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReliefState {
    /// How far the surface moves where the texture is white, millimetres. Negative sinks
    /// the relief in rather than raising it.
    pub amplitude_mm: Scalar,
    pub precision: Scalar,
}

/// What the Relief tool is set to, and the run it has going.
///
/// A texture is pressed into the model as geometry rather than written as exposure: grey
/// controls cure depth, so it could only texture a face pointing up or down; see
/// `docs/decisions/0116-a-texture-is-pressed-into-the-field.md`.
#[derive(Debug)]
pub struct ReliefTool {
    pub state: ReliefState,
    pub job: Option<ReliefJob>,
    /// The picture pressed instead of the models' own textures, while one is chosen. The
    /// window's own, like the tiling: a project does not write it down.
    pub picture: Option<Picture>,
    pub tiling: Tiling,
    /// The picture laid over each model it would be pressed into, for the view to draw.
    laid: HashMap<ObjectId, (LaidFor, Arc<Mapped>)>,
}

impl Default for ReliefTool {
    fn default() -> Self {
        let asked = ReliefSettings::default();
        Self {
            state: ReliefState {
                amplitude_mm: asked.amplitude_mm,
                precision: asked.precision,
            },
            job: None,
            picture: None,
            tiling: Tiling::default(),
            laid: HashMap::new(),
        }
    }
}

impl ReliefTool {
    pub fn settings(&self) -> ReliefSettings {
        ReliefSettings {
            amplitude_mm: self.state.amplitude_mm,
            precision: self.state.precision,
            ..ReliefSettings::default()
        }
    }

    /// Why the Press button is greyed out, or `None` when it is not.
    pub fn blocker(&self, scene: &Scene) -> Option<&'static str> {
        if scene.target_count() == 0 {
            return Some("Nothing visible on the plate to press a texture into.");
        }
        if self.picture.is_none() && relief_tasks(scene, false).is_empty() {
            return Some(
                "None of these models carries a texture. Choose a picture to tile over them, \
                 or open an OBJ with its image beside it, or a 3MF with one inside it.",
            );
        }
        None
    }

    /// Starts a run over everything visible on the plate that carries a texture.
    pub fn start(&mut self, scene: &Scene) -> Result<()> {
        let tasks = relief_tasks(scene, self.picture.is_some());
        if tasks.is_empty() {
            bail!("nothing on the plate carries a texture to press in");
        }
        let source = match &self.picture {
            Some(picture) => ReliefSource::Tiled {
                image: Arc::clone(&picture.heights),
                tiling: self.tiling,
            },
            None => ReliefSource::Mapped,
        };

        self.job = Some(ReliefJob::spawn(ReliefRequest {
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
        match outcome {
            ReliefOutcome::Pressed(models) => {
                let count = models.len();
                let coarsened = models.iter().filter(|model| model.coarsened).count();
                for model in models {
                    if let Some(object) = scene.get_mut(model.id) {
                        // The relief was pressed at printed size, so the scale and the
                        // stretch are in the mesh now and must not be applied twice.
                        object.reshape(model.mesh);
                        object.transform.scale = Vec3::ONE;
                        object.transform.stretch = Mat3::IDENTITY;
                    }
                }
                let depth = self.state.amplitude_mm;
                *status = match coarsened {
                    0 => Status::Info(format!(
                        "Pressed {depth:.2} mm of relief into {count} model(s)"
                    )),
                    _ => Status::Info(format!(
                        "Pressed {depth:.2} mm of relief into {count} model(s); {coarsened} came \
                         out on a coarser lattice than precision asked for"
                    )),
                };
            }
            ReliefOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }

    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }

    /// The texture each model on the plate is drawn with: the chosen picture over the
    /// models it would be pressed into, else the texture each model carries.
    pub fn textures(&mut self, scene: &Scene) -> Textures {
        let Some(picture) = &self.picture else {
            self.laid.clear();
            return scene
                .printable(scene.active_plate())
                .filter_map(|object| Some((object.id, Arc::clone(object.mapped.as_ref()?))))
                .collect();
        };
        let mut laid = HashMap::new();
        for object in scene.targets() {
            let key = LaidFor {
                mesh: Arc::as_ptr(&object.mesh) as usize,
                picture: Arc::as_ptr(&picture.heights) as usize,
                sizing: object.transform.sizing().linear(),
                tiling: self.tiling,
            };
            let mapped = match self.laid.remove(&object.id) {
                Some((was, mapped)) if was == key => mapped,
                _ => Arc::new(lay(picture, &object.mesh, key.sizing, self.tiling)),
            };
            laid.insert(object.id, (key, mapped));
        }
        self.laid = laid;
        self.laid
            .iter()
            .map(|(id, (_, mapped))| (*id, Arc::clone(mapped)))
            .collect()
    }
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
    let read = read_picture(file).with_context(|| format!("cannot read {}", path.display()));
    if let Some(picture) = status.report(&format!("Pressing {}", picture_name(file)), read) {
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
    let image = image::load_from_memory(&bytes)?;
    let size_px = [image.width(), image.height()];
    let image = if size_px[0].max(size_px[1]) > MAX_PICTURE_PX {
        image.thumbnail(MAX_PICTURE_PX, MAX_PICTURE_PX)
    } else {
        image
    };
    let grey = image.to_luma32f();
    let (width, height) = (grey.width() as usize, grey.height() as usize);
    let heights =
        Heightmap::new(width, height, grey.into_raw()).context("the picture has no pixels")?;
    Ok(Picture {
        name: picture_name(file),
        heights: Arc::new(heights),
        size_px,
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
        });
        assert_eq!(tool.blocker(&scene), None);
    }

    #[test]
    fn an_empty_plate_says_there_is_nothing_to_press_into() {
        let mut tool = ReliefTool::default();
        let scene = Scene::default();
        assert_eq!(
            tool.blocker(&scene),
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
            },
            ..ReliefTool::default()
        };
        let settings = tool.settings();
        assert!((settings.amplitude_mm + 0.3).abs() < 1e-6);
        assert!((settings.precision - 0.8).abs() < 1e-6);
    }
}

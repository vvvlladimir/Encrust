use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use anyhow::Result;
use core_geometry::{Mesh, PlacedHit, Scalar, transform_mesh};
use core_text::{Font, Frame, Label, LabelSettings, Shape, TextError, label, outline};
use core_volume::Boolean;

use crate::i18n::tr;
use crate::job::{BooleanJob, BooleanOutcome, BooleanRequest};
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::status::Status;

/// The family the tool writes in until another is picked: the window's own, compiled in.
const BUNDLED_FAMILY: &str = "IBM Plex Sans";

/// A family the tool can write in, and where its regular face is.
#[derive(Debug, Clone, PartialEq)]
pub struct Family {
    pub name: String,
    source: Source,
}

#[derive(Debug, Clone, PartialEq)]
enum Source {
    Bundled,
    File { path: PathBuf, index: u32 },
}

/// The families on offer: the bundled one at once, the machine's own once a worker has
/// read them, since reading every installed face takes a while.
#[derive(Debug)]
pub struct Families {
    pub list: Vec<Family>,
    arriving: Option<Receiver<Vec<Family>>>,
}

impl Default for Families {
    fn default() -> Self {
        let bundled = Family {
            name: BUNDLED_FAMILY.to_owned(),
            source: Source::Bundled,
        };
        Self {
            list: vec![bundled],
            arriving: None,
        }
    }
}

impl Families {
    /// Starts reading the machine's families, the first time it is asked.
    pub fn ask(&mut self) {
        if self.arriving.is_some() || self.list.len() > 1 {
            return;
        }
        let (sender, arriving) = mpsc::channel();
        crate::job::spawn(move || {
            let _ = sender.send(installed());
        });
        self.arriving = Some(arriving);
    }

    /// Takes the machine's families in if they have arrived.
    fn poll(&mut self) {
        let Some(found) = self
            .arriving
            .as_ref()
            .and_then(|arriving| arriving.try_recv().ok())
        else {
            return;
        };
        self.arriving = None;
        self.list.extend(
            found
                .into_iter()
                .filter(|family| family.name != BUNDLED_FAMILY),
        );
    }

    pub fn is_reading(&self) -> bool {
        self.arriving.is_some()
    }
}

/// The installed families, each by its regular face, sorted by name.
#[cfg(not(target_arch = "wasm32"))]
fn installed() -> Vec<Family> {
    let mut database = fontdb::Database::new();
    database.load_system_fonts();
    let mut families: Vec<(Family, bool)> = Vec::new();
    for face in database.faces() {
        let (Some((name, _)), fontdb::Source::File(path)) = (face.families.first(), &face.source)
        else {
            continue;
        };
        let regular = face.style == fontdb::Style::Normal && face.weight == fontdb::Weight::NORMAL;
        let family = Family {
            name: name.clone(),
            source: Source::File {
                path: path.clone(),
                index: face.index,
            },
        };
        match families.iter_mut().find(|(known, _)| known.name == *name) {
            Some(known) if !known.1 && regular => *known = (family, true),
            Some(_) => {}
            None => families.push((family, regular)),
        }
    }
    let mut families: Vec<Family> = families.into_iter().map(|(family, _)| family).collect();
    families.sort_by_key(|family| family.name.to_lowercase());
    families
}

/// A browser offers a page no installed font, so the bundled family is all there is.
#[cfg(target_arch = "wasm32")]
fn installed() -> Vec<Family> {
    Vec::new()
}

/// What the Text section of the Relief panel writes and how. The window's own record.
#[derive(Debug, Clone, PartialEq)]
pub struct TextState {
    pub text: String,
    /// Index into the families on offer.
    pub family: usize,
    /// How tall the capitals stand, millimetres.
    pub height_mm: Scalar,
    /// How far the letters stand out of the surface or into it, millimetres.
    pub depth_mm: Scalar,
    pub raised: bool,
    /// How far the line is turned about the surface's normal, degrees anticlockwise.
    pub angle_deg: Scalar,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            text: "Encrust".to_owned(),
            family: 0,
            height_mm: 6.0,
            depth_mm: 0.8,
            raised: true,
            angle_deg: 0.0,
        }
    }
}

/// The text outlined for one state of the tool, kept until the text, family or height
/// changes.
#[derive(Debug)]
struct Outlined {
    text: String,
    family: Family,
    height_mm: Scalar,
    shapes: Result<Arc<Vec<Shape>>, TextError>,
}

/// Raised or sunk text stamped where the model is clicked; see `docs/design/labels.md`.
#[derive(Debug, Default)]
pub struct TextTool {
    pub state: TextState,
    /// Whether a click on a model writes the text there rather than picking the model.
    pub placing: bool,
    pub families: Families,
    pub job: Option<BooleanJob>,
    /// Behind a cell so the view, which only reads the tool, can outline what it draws.
    outlined: RefCell<Option<Outlined>>,
}

impl TextTool {
    /// The text outlined in the family and height the tool is set to.
    pub fn shapes(&self) -> Result<Arc<Vec<Shape>>, TextError> {
        let family = self
            .families
            .list
            .get(self.state.family)
            .or(self.families.list.first())
            .cloned()
            .ok_or(TextError::Blank)?;
        let mut outlined = self.outlined.borrow_mut();
        let current = outlined.as_ref().is_some_and(|outlined| {
            outlined.text == self.state.text
                && outlined.family == family
                && outlined.height_mm == self.state.height_mm
        });
        if !current {
            let shapes = font_of(&family)
                .and_then(|font| outline(&font, &self.state.text, self.state.height_mm))
                .map(Arc::new);
            *outlined = Some(Outlined {
                text: self.state.text.clone(),
                family,
                height_mm: self.state.height_mm,
                shapes,
            });
        }
        outlined
            .as_ref()
            .map_or(Err(TextError::Blank), |outlined| outlined.shapes.clone())
    }

    /// The label a click at `hit` on `object` would stamp, on the plate.
    pub fn label_at(&self, object: &SceneObject, hit: &PlacedHit) -> Result<Label, TextError> {
        let shapes = self.shapes()?;
        let frame =
            Frame::on(hit.point, hit.normal, self.state.angle_deg).ok_or(TextError::OffTheModel)?;
        label(
            &shapes,
            frame,
            &object.mesh,
            &object.bvh,
            object.transform,
            LabelSettings {
                depth_mm: self.state.depth_mm,
                raised: self.state.raised,
            },
        )
    }

    /// Stamps the label at `hit` into `object`, off the drawing thread.
    pub fn stamp(&mut self, scene: &Scene, id: ObjectId, hit: &PlacedHit) -> Result<()> {
        let object = scene
            .get(id)
            .ok_or_else(|| anyhow::anyhow!(tr!("text-nothing-under")))?;
        let label = self
            .label_at(object, hit)
            .map_err(|error| anyhow::anyhow!(reason(&error)))?;
        let mut placed = vec![transform_mesh(&object.mesh, object.transform)];
        placed.extend(label.prisms);
        self.job = Some(BooleanJob::spawn(BooleanRequest {
            ids: vec![id],
            name: object.name.clone(),
            placed,
            op: if label.raised {
                Boolean::Union
            } else {
                Boolean::Difference
            },
            // Lettering must not remesh the model it is written on.
            exact: true,
        }));
        Ok(())
    }

    /// Lands a finished stamp in the scene. Returns whether one is still going.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        self.families.poll();
        let Some(outcome) = self.job.as_mut().and_then(BooleanJob::poll) else {
            return self.job.is_some() || self.families.is_reading();
        };
        self.job = None;
        *status = match outcome {
            BooleanOutcome::Combined {
                ids, name, mesh, ..
            } => {
                let mending = crate::boolean::land(scene, &ids, name.clone(), mesh);
                Status::Info(tr!("text-done", name = name, mending = mending))
            }
            BooleanOutcome::Failed(message) => Status::Error(message),
        };
        false
    }

    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }
}

/// The whole label as one mesh, for the view to draw before it is stamped.
pub fn ghost_mesh(label: &Label) -> Mesh {
    let mut whole = Mesh::default();
    for prism in &label.prisms {
        whole.append(prism);
    }
    whole
}

fn font_of(family: &Family) -> Result<Font, TextError> {
    let data = match &family.source {
        Source::Bundled => crate::ui::fonts::SANS.to_vec(),
        Source::File { path, .. } => {
            std::fs::read(path).map_err(|error| TextError::BadFont(error.to_string()))?
        }
    };
    let index = match &family.source {
        Source::Bundled => 0,
        Source::File { index, .. } => *index,
    };
    Font::new(Arc::new(data), index)
}

/// What the panel and the stage notice say about a label that cannot be made.
pub fn reason(error: &TextError) -> String {
    match error {
        TextError::BadFont(why) => tr!("text-bad-font", reason = why.clone()),
        TextError::MissingGlyph(character) => {
            tr!("text-missing-glyph", character = character.to_string())
        }
        TextError::Blank => tr!("text-blank"),
        TextError::BadSize { .. } => tr!("text-bad-size"),
        TextError::OffTheModel => tr!("text-off-the-model"),
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Transform, Vec3, cuboid, diagnose, signed_volume};

    use super::*;
    use crate::scene::{ImportSummary, Imported};

    /// A 40 mm cube on the plate, and a click on the middle of its top face.
    fn cube() -> (Scene, ObjectId, PlacedHit) {
        let mesh = cuboid(Vec3::ZERO, Vec3::splat(40.0));
        let mut scene = Scene::default();
        let id = scene.insert(Imported::new(
            "Cube".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: core_geometry::Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));
        let hit = PlacedHit {
            point: Vec3::new(20.0, 20.0, 40.0),
            normal: Vec3::Z,
            distance: 0.0,
            face: 0,
        };
        (scene, id, hit)
    }

    fn stamped(raised: bool) -> (Status, f32) {
        let (mut scene, id, hit) = cube();
        let mut tool = TextTool::default();
        tool.state.raised = raised;
        tool.stamp(&scene, id, &hit)
            .expect("the text stands on the cube");
        let mut status = Status::default();
        while tool.poll(&mut scene, &mut status) && tool.is_busy() {
            std::thread::yield_now();
        }
        let [object] = scene.objects() else {
            panic!("the cube is replaced by one model");
        };
        (status, signed_volume(&object.mesh))
    }

    #[test]
    fn raised_text_adds_to_the_model_and_sunk_text_takes_from_it() {
        let (status, raised) = stamped(true);
        assert!(matches!(status, Status::Info(_)), "{status:?}");
        assert!(
            raised > 64_000.0,
            "raised letters add to the 64000 mm³ cube; got {raised}"
        );
        let (status, sunk) = stamped(false);
        assert!(matches!(status, Status::Info(_)), "{status:?}");
        assert!(sunk < 64_000.0, "sunk letters take from it; got {sunk}");
    }

    #[test]
    fn letters_on_a_closed_model_are_cut_exactly() {
        let (scene, id, hit) = cube();
        let tool = TextTool::default();
        let object = scene.get(id).expect("the cube");
        let label = tool
            .label_at(object, &hit)
            .expect("the text stands on the cube");
        let mut models = vec![transform_mesh(&object.mesh, object.transform)];
        models.extend(label.prisms);
        let combined = core_volume::combine(&models, Boolean::Union, core_volume::Cancel::never())
            .expect("the letters combine with the cube");
        assert_eq!(combined.voxel_mm, None, "every letter is a closed solid");
    }

    #[test]
    fn text_on_a_model_that_is_not_closed_is_refused_and_leaves_it_be() {
        let (mut scene, id, hit) = cube();
        let mut open = scene.get(id).expect("the cube").mesh.as_ref().clone();
        open.faces.truncate(10);
        let before = open.clone();
        scene
            .get_mut(id)
            .expect("the cube")
            .reshape(crate::scene::Shaped::of(Arc::new(open)));
        let mut tool = TextTool::default();
        tool.stamp(&scene, id, &hit)
            .expect("the text stands on what is left");
        let mut status = Status::default();
        while tool.poll(&mut scene, &mut status) && tool.is_busy() {
            std::thread::yield_now();
        }
        assert_eq!(status, Status::Error(tr!("boolean-not-solid")));
        assert_eq!(
            *scene.get(id).expect("still there").mesh,
            before,
            "never remeshed"
        );
    }

    #[test]
    fn a_click_beside_the_model_writes_nothing() {
        let (scene, id, mut hit) = cube();
        hit.point = Vec3::new(500.0, 500.0, 40.0);
        let mut tool = TextTool::default();
        assert!(tool.stamp(&scene, id, &hit).is_err());
        assert!(!tool.is_busy());
    }
}

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use core_engine::Model;
use core_geometry::Vec3;
use core_thumbnail::{Marker, Part, Rendered, Rgb, ThumbnailSettings, View, render_view};
use serde::Serialize;

use crate::args::JobArgs;
use crate::commands::GlobalArgs;
use crate::commands::slice::PlateArgs;
use crate::exit::{Exit, Stop};
use crate::hollowing::point;
use crate::{json, stage};

#[derive(clap::Args, Debug)]
pub struct RenderCommand {
    #[command(flatten)]
    pub plate: PlateArgs,

    /// Where the pictures go: a `.png` name for one view, otherwise a directory holding one
    /// `<view>.png` per view.
    #[arg(short, long, default_value = "render")]
    pub output: PathBuf,

    /// The sides to look from, comma separated: iso, front, back, left, right, top, bottom.
    #[arg(long, value_name = "VIEWS", value_delimiter = ',', value_parser = view,
          default_values = ["iso", "bottom"])]
    pub view: Vec<View>,

    /// Width and height of each picture, pixels.
    #[arg(long, value_name = "PX", default_value_t = 512,
          value_parser = clap::value_parser!(u32).range(16..=4096))]
    pub size: u32,

    /// Mark this point on every picture, plate mm from the front left corner. May repeat.
    #[arg(long, value_name = "X,Y,Z", value_parser = point, allow_hyphen_values = true)]
    pub mark: Vec<Vec3>,

    #[command(flatten)]
    pub job: JobArgs,
}

/// What a marker is drawn in: orange reads on the dark background and on the blue model.
const MARK: Rgb = [0xFF, 0x7A, 0x1A];

impl RenderCommand {
    pub fn run(&self, global: &GlobalArgs, stop: &Stop) -> Result<Exit> {
        self.plate.check()?;
        let targets = self.targets()?;
        let watch = global.watch(stop);
        let plate = &self.plate;
        let staged = stage::stage(&plate.inputs, plate.arrange, plate.plate, &self.job, &watch)?;
        if self.job.strict && !staged.parts.iter().all(stage::Part::is_clean) {
            report(global, &[])?;
            return Ok(Exit::Unclean);
        }
        let models = drilled(staged.models, stop)?;
        let placed = core_engine::parts(&models);
        let parts: Vec<Part<'_>> = placed
            .iter()
            .map(|(mesh, transform)| Part::new(mesh, *transform))
            .collect();
        let markers = self.markers();
        let settings = ThumbnailSettings {
            width_px: self.size,
            height_px: self.size,
            ..ThumbnailSettings::default()
        };
        let mut pictures = Vec::with_capacity(targets.len());
        for (view, path) in targets {
            stop.check()?;
            let rendered = render_view(&parts, view, &markers, &settings);
            write_png(&rendered, &path)?;
            pictures.push(Picture::of(view, path, &rendered, &markers));
        }
        report(global, &pictures)?;
        Ok(Exit::Success)
    }

    /// Every point to mark, drawn alike.
    fn markers(&self) -> Vec<Marker> {
        // TODO(step-7): --mark-findings, a mark per slicing.trapped, cured.risks and no_room_at_mm
        // point; render cuts no stack, so it needs the check's slice first, here.
        self.mark
            .iter()
            .map(|&at| Marker { at, colour: MARK })
            .collect()
    }

    /// Each view asked for, once, with the file it is written to.
    fn targets(&self) -> Result<Vec<(View, PathBuf)>> {
        let mut views: Vec<View> = Vec::with_capacity(self.view.len());
        for view in &self.view {
            if !views.contains(view) {
                views.push(*view);
            }
        }
        let extension = self.output.extension().and_then(|text| text.to_str());
        match (
            extension.map(str::to_ascii_lowercase).as_deref(),
            views.as_slice(),
        ) {
            (Some("png"), [view]) => Ok(vec![(*view, self.output.clone())]),
            (Some("png"), _) => bail!(
                "{} is one picture and --view asks for {}; name a directory for several",
                self.output.display(),
                views.len()
            ),
            (Some(other), _) => {
                bail!("render writes PNG pictures, not .{other}: name a .png file or a directory")
            }
            (None, _) => Ok(views
                .into_iter()
                .map(|view| (view, self.output.join(format!("{}.png", view.name()))))
                .collect()),
        }
    }
}

/// One `--view` value, by the name the picture's file takes.
fn view(text: &str) -> Result<View, String> {
    View::named(text.trim()).ok_or_else(|| {
        let names: Vec<&str> = View::ALL.iter().map(|view| view.name()).collect();
        format!("no view {text:?}; one of {}", names.join(", "))
    })
}

/// The models with their drain holes and channels taken out, so a picture shows the holes. A
/// model the engine cannot drill exactly is drawn whole: the slice takes them out by winding.
fn drilled(models: Vec<Model>, stop: &Stop) -> Result<Vec<Model>> {
    models
        .into_iter()
        .map(|mut model| {
            stop.check()?;
            if let (Some(cuts), None) = (&model.cuts, &model.drilled) {
                match core_volume::drilled(&model.mesh, cuts) {
                    Ok(body) => model.drilled = Some(Arc::new(body)),
                    Err(error) => tracing::warn!("the holes are not drawn: {error}"),
                }
            }
            Ok(model)
        })
        .collect()
}

/// Writes one picture, making the directory it goes in once the plate has staged.
fn write_png(rendered: &Rendered, path: &Path) -> Result<()> {
    let bytes = core_format::encode_colour(&rendered.image)
        .with_context(|| format!("cannot encode {}", path.display()))?;
    if let Some(directory) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(directory)
            .with_context(|| format!("cannot make {}", directory.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}

fn report(global: &GlobalArgs, pictures: &[Picture]) -> Result<()> {
    if global.json {
        return json::print(&serde_json::json!({ "pictures": pictures }));
    }
    if global.talks() && !pictures.is_empty() {
        println!("Rendered");
        for picture in pictures {
            println!("  {:<15}{}", picture.view, picture.path.display());
        }
    }
    Ok(())
}

/// One picture written, as `--json` lists it.
#[derive(Serialize)]
struct Picture {
    view: &'static str,
    path: PathBuf,
    width_px: u32,
    height_px: u32,
    markers: Vec<Spot>,
}

/// Where one `--mark` landed in a picture.
#[derive(Serialize)]
struct Spot {
    at_mm: [f32; 3],
    x_px: f32,
    y_px: f32,
    hidden: bool,
    /// Off the picture, which frames the models alone, so drawn at the edge nearest it.
    pinned: bool,
}

impl Picture {
    fn of(view: View, path: PathBuf, rendered: &Rendered, markers: &[Marker]) -> Self {
        let markers = markers
            .iter()
            .zip(&rendered.markers)
            .map(|(marker, spot)| Spot {
                at_mm: marker.at.to_array(),
                x_px: spot.x_px,
                y_px: spot.y_px,
                hidden: spot.hidden,
                pinned: spot.pinned,
            })
            .collect();
        Self {
            view: view.name(),
            path,
            width_px: rendered.image.width_px(),
            height_px: rendered.image.height_px(),
            markers,
        }
    }
}

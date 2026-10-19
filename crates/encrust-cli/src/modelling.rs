//! What the window's Combine, Text, Cut and Split do, done to the models on the command line
//! after they are placed and before they are hollowed; see docs/decisions/0312.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use core_geometry::{Bvh, Mesh, Plane, Quat, Scalar, Transform, Vec3, transform_mesh};
use core_joint::{CutJoined, JointKind, Joints, cut_and_join};
use core_text::{Font, Frame, LABEL_DEPTH_MM, LETTER_HEIGHT_MM, LabelSettings, label, outline};
use core_volume::{Boolean, Cancel, PartSettings, combine, combine_exact_toward, part_off, parts};

use crate::hollowing::point;

/// The face labels are written in unless `--label-font` names another, the window's own.
const BUNDLED_FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/IBMPlexSans-Regular.ttf"
));

/// How `--combine` joins the models.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Op {
    Union,
    /// The first model less every other.
    Difference,
    Intersection,
}

impl From<Op> for Boolean {
    fn from(op: Op) -> Self {
        match op {
            Op::Union => Self::Union,
            Op::Difference => Self::Difference,
            Op::Intersection => Self::Intersection,
        }
    }
}

/// Which halves `--cut` keeps.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Keep {
    #[default]
    Both,
    Lower,
    Upper,
}

/// What `--joint` joins the halves of a cut by.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Joint {
    Dowels,
    Pins,
    Magnets,
    Key,
}

impl From<Joint> for JointKind {
    fn from(joint: Joint) -> Self {
        match joint {
            Joint::Dowels => Self::Dowels,
            Joint::Pins => Self::Pins,
            Joint::Magnets => Self::Magnets,
            Joint::Key => Self::Key,
        }
    }
}

/// The modelling half of the command line.
#[derive(Debug, Clone, Default, clap::Args)]
#[command(next_help_heading = "Modelling")]
pub struct ModelArgs {
    /// Combine every model on the command line into one.
    #[arg(long, value_enum, value_name = "OP")]
    pub combine: Option<Op>,

    /// Write this text on the model, on the surface nearest --label-at.
    #[arg(long, value_name = "TEXT", requires = "label_at")]
    pub label: Option<String>,

    /// The point on the plate, millimetres, the label is written nearest to.
    #[arg(long, value_name = "X,Y,Z", value_parser = point, allow_hyphen_values = true,
          requires = "label")]
    pub label_at: Option<Vec3>,

    /// How tall the label's capitals stand, millimetres.
    #[arg(long, value_name = "MM", default_value_t = LETTER_HEIGHT_MM, requires = "label")]
    pub label_height: Scalar,

    /// How far the letters stand out of the surface, or into it, millimetres.
    #[arg(long, value_name = "MM", default_value_t = LABEL_DEPTH_MM, requires = "label")]
    pub label_depth: Scalar,

    /// Sink the letters into the surface rather than raise them out of it.
    #[arg(long, requires = "label")]
    pub label_sunk: bool,

    /// Bend every letter onto the surface under it rather than stand it flat.
    #[arg(long, requires = "label")]
    pub label_follow: bool,

    /// How far the line of text is turned about the surface's normal, degrees anticlockwise.
    #[arg(
        long,
        value_name = "DEG",
        default_value_t = 0.0,
        allow_hyphen_values = true,
        requires = "label"
    )]
    pub label_angle: Scalar,

    /// A font file to write the label in, in place of the bundled IBM Plex Sans.
    #[arg(long, value_name = "PATH", requires = "label")]
    pub label_font: Option<PathBuf>,

    /// Cut the model with a plane square to an axis at a plate coordinate, millimetres:
    /// `z:10` is 10 mm over the plate.
    #[arg(long, value_name = "AXIS:MM", value_parser = plane, allow_hyphen_values = true)]
    pub cut: Option<Plane>,

    /// Which halves of the cut are kept.
    #[arg(long, value_enum, default_value_t = Keep::Both, requires = "cut")]
    pub keep: Keep,

    /// Join the two halves of the cut, at the window's default sizes.
    #[arg(long, value_enum, requires = "cut")]
    pub joint: Option<Joint>,

    /// Break each model into the bodies it is made of.
    #[arg(long, conflicts_with = "part_off")]
    pub split: bool,

    /// A piece holding less than this is a scrap, not a part of its own, cubic millimetres.
    #[arg(long, value_name = "MM3", default_value_t = PartSettings::default().smallest_mm3,
          requires = "split")]
    pub split_smallest: Scalar,

    /// Cut off the part of the model this point on the plate is on, flat where it narrows
    /// into the rest.
    #[arg(long, value_name = "X,Y,Z", value_parser = point, allow_hyphen_values = true)]
    pub part_off: Option<Vec3>,
}

impl ModelArgs {
    /// Whether anything is asked of the models.
    pub fn wanted(&self) -> bool {
        self.combine.is_some()
            || self.label.is_some()
            || self.cut.is_some()
            || self.split
            || self.part_off.is_some()
    }

    /// Whether a model may come out as several pieces, each to be put down on its own.
    pub fn splits(&self) -> bool {
        self.cut.is_some() || self.split || self.part_off.is_some()
    }
}

/// `axis:mm` as the plane square to that axis of the plate at that coordinate.
fn plane(text: &str) -> Result<Plane, String> {
    let (axis, at) = text
        .split_once(':')
        .ok_or_else(|| format!("a cut is AXIS:MM, such as z:10, got {text:?}"))?;
    let normal = match axis.trim().to_ascii_lowercase().as_str() {
        "x" => Vec3::X,
        "y" => Vec3::Y,
        "z" => Vec3::Z,
        other => return Err(format!("a cut is across x, y or z, got {other:?}")),
    };
    let at: Scalar = at.trim().parse().map_err(|error| format!("{error}"))?;
    Plane::at(normal, at).ok_or_else(|| format!("{text:?} names no plane"))
}

/// The models as `args` shapes them, each into the pieces it becomes, in plate coordinates.
/// Combining leaves the first the whole result and the others nothing.
pub fn shaped(models: Vec<Mesh>, args: &ModelArgs, talk: bool) -> Result<Vec<Vec<Mesh>>> {
    let models = match args.combine {
        Some(op) => combined(models, op, talk)?,
        None => models,
    };
    models
        .into_iter()
        .map(|mesh| {
            if mesh.faces.is_empty() {
                return Ok(Vec::new());
            }
            let mesh = match &args.label {
                Some(text) => labelled(&mesh, text, args, talk)?,
                None => mesh,
            };
            let halves = match args.cut {
                Some(plane) => cut(&mesh, plane, args, talk)?,
                None => vec![mesh],
            };
            let mut pieces = Vec::new();
            for half in halves {
                pieces.extend(parted(half, args, talk)?);
            }
            Ok(pieces)
        })
        .collect()
}

/// Every model as one, in the first one's place; the others are left empty.
fn combined(models: Vec<Mesh>, op: Op, talk: bool) -> Result<Vec<Mesh>> {
    if models.len() < 2 {
        bail!("--combine takes two models or more, got {}", models.len());
    }
    let joined = combine(&models, op.into(), Cancel::never()).context("combining the models")?;
    if talk {
        match joined.voxel_mm {
            None => println!("Combined {} models\n", models.len()),
            Some(voxel_mm) => println!(
                "Combined {} models on a {voxel_mm:.2} mm lattice, as one is not a closed solid\n",
                models.len()
            ),
        }
    }
    let mut out = vec![joined.mesh];
    out.resize_with(models.len(), Mesh::default);
    Ok(out)
}

/// `mesh` with `text` written on the surface nearest `--label-at`.
fn labelled(mesh: &Mesh, text: &str, args: &ModelArgs, talk: bool) -> Result<Mesh> {
    let at = args.label_at.context("--label needs --label-at")?;
    let font = match &args.label_font {
        Some(path) => {
            let bytes = std::fs::read(path)
                .with_context(|| format!("cannot read the font {}", path.display()))?;
            Font::new(Arc::new(bytes), 0)
        }
        None => Font::new(Arc::new(BUNDLED_FONT.to_vec()), 0),
    }
    .context("reading the font")?;
    let shapes = outline(&font, text, args.label_height).context("outlining the label")?;
    let bvh = Bvh::build(mesh);
    let nearest = bvh.closest(mesh, at).context("the model has no surface")?;
    let normal = mesh
        .triangle(nearest.face)
        .map(|triangle| triangle.normal_unnormalized())
        .context("the nearest face has no area")?;
    let frame = Frame::on(nearest.point, normal, args.label_angle)
        .context("the nearest face has no direction")?;
    let settings = LabelSettings {
        depth_mm: args.label_depth,
        raised: !args.label_sunk,
        follow_surface: args.label_follow,
    };
    let written = label(&shapes, frame, mesh, &bvh, Transform::default(), settings)
        .context("laying the label on the model")?;
    let op = if written.raised {
        Boolean::Union
    } else {
        Boolean::Difference
    };
    let mut models = vec![mesh.clone()];
    models.extend(written.prisms);
    let stamped =
        combine_exact_toward(&models, op, frame.normal).context("writing the label in")?;
    if talk {
        println!("Labelled with {text:?} at {}\n", nearest.point);
    }
    Ok(stamped)
}

/// The halves `plane` cuts `mesh` into that `--keep` keeps, a half no longer reaching down
/// to where the model stood laid on its cut face, and the loose dowels a dowel joint prints.
fn cut(mesh: &Mesh, plane: Plane, args: &ModelArgs, talk: bool) -> Result<Vec<Mesh>> {
    let joints = args
        .joint
        .filter(|_| args.keep == Keep::Both)
        .map(|joint| Joints {
            kind: Some(joint.into()),
            ..Joints::default()
        });
    let Some(CutJoined {
        joined,
        open_loops,
        unjoined,
    }) = cut_and_join(mesh, None, plane, joints.as_ref())
    else {
        bail!("the --cut plane misses the model");
    };
    if let Some(reason) = unjoined {
        bail!("the joint could not be built into the halves: {reason}");
    }
    if talk {
        println!(
            "Cut: {} fixings, {} open loops left uncapped\n",
            joined.placed, open_loops
        );
    }
    let floor = mesh.aabb().map_or(0.0, |bounds| bounds.mins.z);
    let lower = stood(joined.below, plane.normal, floor);
    let upper = stood(joined.above, -plane.normal, floor);
    let mut kept = match args.keep {
        Keep::Both => vec![lower, upper],
        Keep::Lower => vec![lower],
        Keep::Upper => vec![upper],
    };
    kept.extend(joined.loose.into_iter().map(|dowel| dowel.mesh));
    Ok(kept)
}

/// How far over where the model stood a half's lowest point may be and still stand there,
/// millimetres: rounding, not a gap.
const STANDS_MM: Scalar = 1e-3;

/// `half` as it is where it still reaches down to `floor`, or else turned so its cut face,
/// looking along `facing`, lies on the plate.
fn stood(half: Mesh, facing: Vec3, floor: Scalar) -> Mesh {
    if half
        .aabb()
        .is_none_or(|bounds| bounds.mins.z <= floor + STANDS_MM)
    {
        return half;
    }
    let turned = transform_mesh(
        &half,
        Transform {
            rotation: Quat::from_rotation_arc(facing, -Vec3::Z),
            ..Transform::default()
        },
    );
    let low = turned.aabb().map_or(0.0, |bounds| bounds.mins.z);
    transform_mesh(
        &turned,
        Transform::from_translation(Vec3::Z * (floor - low)),
    )
}

/// `mesh` as `--split` or `--part-off` leaves it, or whole.
fn parted(mesh: Mesh, args: &ModelArgs, talk: bool) -> Result<Vec<Mesh>> {
    if let Some(at) = args.part_off {
        let parted = part_off(&mesh, at, Cancel::never()).context("cutting the part off")?;
        if talk {
            println!("Cut the part at {at} off where it joins the rest\n");
        }
        return Ok(vec![parted.part, parted.rest]);
    }
    if !args.split {
        return Ok(vec![mesh]);
    }
    let settings = PartSettings {
        smallest_mm3: args.split_smallest,
    };
    let found = parts(&mesh, &settings);
    if talk {
        println!(
            "Split into {} parts: {} scraps joined, {} dropped\n",
            found.parts.len(),
            found.joined,
            found.dropped
        );
    }
    Ok(found.parts)
}

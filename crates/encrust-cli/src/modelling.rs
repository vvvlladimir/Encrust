//! What the window's Combine, Text, Cut and Split do, done to the models on the command line
//! after they are placed and before they are hollowed; see docs/decisions/0312.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use core_document::{ApplyError, CutAsked, CutModel, LabelAsked, part_off_at, pieces, split};
use core_edit::{Axis, ObjectId};
use core_geometry::{Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_joint::{JointKind, Joints};
use core_text::{LABEL_DEPTH_MM, LETTER_HEIGHT_MM};
use core_volume::{Boolean, Cancel, PartSettings};

use crate::hollowing::point;

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

impl From<Keep> for core_edit::Keep {
    fn from(keep: Keep) -> Self {
        match keep {
            Keep::Both => Self::Both,
            Keep::Lower => Self::Below,
            Keep::Upper => Self::Above,
        }
    }
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
    #[arg(long, value_name = "AXIS:MM", value_parser = cut_at, allow_hyphen_values = true)]
    pub cut: Option<(Axis, Scalar)>,

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

/// `axis:mm` as the axis of the plate a cut is square to and the coordinate it crosses at.
fn cut_at(text: &str) -> Result<(Axis, Scalar), String> {
    let (axis, at) = text
        .split_once(':')
        .ok_or_else(|| format!("a cut is AXIS:MM, such as z:10, got {text:?}"))?;
    let axis = match axis.trim().to_ascii_lowercase().as_str() {
        "x" => Axis::X,
        "y" => Axis::Y,
        "z" => Axis::Z,
        other => return Err(format!("a cut is across x, y or z, got {other:?}")),
    };
    let at: Scalar = at.trim().parse().map_err(|error| format!("{error}"))?;
    if !at.is_finite() {
        return Err(format!("{text:?} names no plane"));
    }
    Ok((axis, at))
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
    let joined =
        core_document::combined(&models, op.into(), None, Cancel::never(), ObjectId::UNSET)
            .map_err(|error| match error {
                ApplyError::NotSolid(_) => anyhow!("a model is not a closed solid"),
                other => other.into(),
            })
            .context("combining the models")?;
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
    let at_mm = args.label_at.context("--label needs --label-at")?;
    let asked = LabelAsked {
        text: text.to_owned(),
        at_mm,
        height_mm: args.label_height,
        depth_mm: args.label_depth,
        raised: !args.label_sunk,
        follow_surface: args.label_follow,
        angle_deg: args.label_angle,
        font: args.label_font.clone(),
    };
    let stamped = core_document::labelled(mesh, &asked, Cancel::never(), ObjectId::UNSET)
        .map_err(|error| match error {
            ApplyError::NoSurface(_) => anyhow!("the model has no surface near --label-at"),
            ApplyError::NotSolid(_) => anyhow!("the model is not a closed solid"),
            ApplyError::BadFont {
                path: Some(path), ..
            } => anyhow!("cannot read the font {}", path.display()),
            other => other.into(),
        })
        .context("writing the label")?;
    if talk {
        println!("Labelled with {text:?} at {at_mm}\n");
    }
    Ok(stamped)
}

/// The halves `--cut` cuts `mesh` into that `--keep` keeps, each laid on its cut face as
/// the window's Lay flat does, and the loose dowels a dowel joint prints (ADR 0466).
fn cut(
    mesh: &Mesh,
    (axis, at_mm): (Axis, Scalar),
    args: &ModelArgs,
    talk: bool,
) -> Result<Vec<Mesh>> {
    let asked = CutAsked {
        axis,
        at_mm,
        keep: args.keep.into(),
        joints: Joints {
            kind: args.joint.map(JointKind::from),
            ..Joints::default()
        },
        lay_flat: true,
    };
    let model = CutModel {
        printed: Arc::new(mesh.clone()),
        cuts: None,
        solid: None,
        transform: Transform::default(),
    };
    let Ok(done) = core_document::cut(&model, &asked, ObjectId::UNSET) else {
        bail!("the --cut plane misses the model");
    };
    if let Some(reason) = done.unjoined {
        bail!("the joint could not be built into the halves: {reason}");
    }
    if talk {
        println!(
            "Cut: {} fixings, {} open loops left uncapped\n",
            done.joined.placed, done.open_loops
        );
    }
    let kept = pieces(done.joined, &asked);
    Ok(kept
        .into_iter()
        .map(|piece| transform_mesh(&piece.mesh, piece.transform))
        .collect())
}

/// `mesh` as `--split` or `--part-off` leaves it, or whole.
fn parted(mesh: Mesh, args: &ModelArgs, talk: bool) -> Result<Vec<Mesh>> {
    if let Some(at) = args.part_off {
        let parted = part_off_at(&mesh, at, Cancel::never(), ObjectId::UNSET)
            .map_err(|error| match error {
                ApplyError::NoNeck(_) => {
                    anyhow!("there is no narrow join to cut at from this point")
                }
                other => other.into(),
            })
            .context("cutting the part off")?;
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
    let found = split(&mesh, &settings);
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

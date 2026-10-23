//! A model stood the way the corpus asks and supported the way the window's Generate
//! supports it, sized by an automatic profile and against its hollowed shell where asked,
//! keeping every tree so the bench can read what each support carries.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use std::sync::Arc;

use core_geometry::{
    Bvh, Mat4, Mesh, Quat, Scalar, Transform, Vec3, center_over_plate, drop_to_plate,
    lift_over_plate, orient_outward, transform_mesh, weld,
};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};
use core_supports::{
    Blocked, Need, Placed, Profiles, SupportPoint, SupportTree, WallSide, columns,
    generate_supports, grow, low_points, mesh_trees,
};
use core_volume::{Cancel, HollowSettings, hollow};
use encrust_cli::pipeline;
use printer_profiles::{AUTO_SIZES, AutoProfile, BuildVolume, SupportProfile};

use crate::corpus::{Pose, Up};

/// What a bench supports a model to: one profile, or the shapes an automatic profile sizes
/// each support into, as Generate does with Automatic on (ADR 0325).
#[derive(Clone)]
pub enum Asked {
    Profile(Box<SupportProfile>),
    Auto {
        auto: Box<AutoProfile>,
        /// Its shapes lightest first, then its steadying one.
        shapes: Box<[SupportProfile; AUTO_SIZES + 1]>,
    },
}

impl Asked {
    /// The profile placement runs with: where supports go and how close.
    pub fn placing(&self) -> SupportProfile {
        match self {
            Self::Profile(profile) => profile.as_ref().clone(),
            Self::Auto { auto, shapes } => auto.placing_with(&shapes[0]),
        }
    }

    /// The same, with `share` of its density.
    pub fn thinned(&self, share: f32) -> Self {
        let mut thinned = self.clone();
        match &mut thinned {
            Self::Profile(profile) => profile.density *= share,
            Self::Auto { auto, .. } => auto.density *= share,
        }
        thinned
    }

    fn table(&self) -> Vec<SupportProfile> {
        match self {
            Self::Profile(profile) => vec![profile.as_ref().clone()],
            Self::Auto { shapes, .. } => shapes.to_vec(),
        }
    }

    fn sizing(&self) -> Option<&AutoProfile> {
        match self {
            Self::Profile(_) => None,
            Self::Auto { auto, .. } => Some(auto.as_ref()),
        }
    }

    fn group_of(&self, need: &Need) -> u16 {
        self.sizing()
            .map_or(0, |auto| u16::try_from(need.size_in(auto)).unwrap_or(0))
    }
}

/// A model on the plate and everything automatic placement stood under it.
pub struct Supported {
    /// The model as it prints, its shell where it was hollowed, lifted and in plate
    /// coordinates.
    pub model: Mesh,
    /// Every contact placement asked for, standing or not.
    pub contacts: Vec<Vec3>,
    /// The group of `profiles` each of `contacts` is built to.
    pub groups: Vec<u16>,
    pub profiles: Vec<SupportProfile>,
    pub trees: Vec<SupportTree>,
    /// The supports as one mesh, plate coordinates.
    pub supports: Mesh,
    pub place_s: f64,
    pub grow_s: f64,
}

impl Supported {
    /// The profile the support at `contact`, an index into `contacts`, is built to.
    pub fn profile_of(&self, contact: usize) -> &SupportProfile {
        let group = self.groups.get(contact).copied().unwrap_or(0);
        &self.profiles[usize::from(group).min(self.profiles.len() - 1)]
    }
}

/// Reads `path` as one closed, outward mesh.
pub fn load(path: &Path) -> Result<Mesh> {
    let loaded = pipeline::load(path)?;
    let mut mesh = weld(&loaded.mesh, core_geometry::DEFAULT_WELD_TOLERANCE).mesh;
    orient_outward(&mut mesh);
    Ok(mesh)
}

/// `mesh` turned up, scaled, spun and leant as `pose` says, set down in the middle of the
/// plate, and the turn that took its upright pose to the printed one.
pub fn posed(mesh: &Mesh, pose: Pose, volume: &BuildVolume) -> Result<(Mesh, Quat)> {
    let half = std::f32::consts::FRAC_PI_2;
    let up = match pose.up {
        Up::Z => Quat::IDENTITY,
        Up::Y => Quat::from_rotation_x(half),
        Up::NegY => Quat::from_rotation_x(-half),
        Up::X => Quat::from_rotation_y(-half),
    };
    let turned = transform_mesh(mesh, rotation(up));
    let bounds = turned.aabb().context("the model has no vertices")?;
    let tall = bounds.maxs.z - bounds.mins.z;
    let scale = match pose.height_mm {
        Some(height) if tall > 0.0 => height / tall,
        _ => 1.0,
    };
    let leant = Quat::from_rotation_x(pose.tilt_deg.to_radians())
        * Quat::from_rotation_z(pose.spin_deg.to_radians());
    let sized = Transform {
        scale: Vec3::splat(scale),
        ..Transform::default()
    };
    let stood = transform_mesh(&transform_mesh(&turned, sized), rotation(leant));
    let bounds = stood.aabb().context("the model has no vertices")?;
    let offset = drop_to_plate(&bounds) + center_over_plate(&bounds, volume.x, volume.y);
    Ok((
        transform_mesh(&stood, Transform::from_translation(offset)),
        leant,
    ))
}

fn rotation(rotation: Quat) -> Transform {
    Transform {
        rotation,
        ..Transform::default()
    }
}

/// `model` lifted by the placing profile's lift, hollowed to a wall of `wall_mm` where one
/// is given, and supported on its own from outside, as Generate supports a model on the
/// plate with nothing painted, blocked or placed by hand.
pub fn supported(
    mut model: Mesh,
    asked: &Asked,
    wall_mm: Option<Scalar>,
    layer_mm: Scalar,
) -> Result<Supported> {
    let placing = asked.placing();
    let bounds = model.aabb().context("the model has no vertices")?;
    if bounds.mins.z < placing.lift_mm() {
        let offset = lift_over_plate(&bounds, placing.lift_mm());
        model = transform_mesh(&model, Transform::from_translation(offset));
    }
    let (model, side) = printed(model, wall_mm)?;
    let bvh = Bvh::build(&model);
    let started = Instant::now();
    let needs = needs_under(&model, &placing, asked.sizing(), side.as_ref(), layer_mm)?;
    let place_s = started.elapsed().as_secs_f64();

    let started = Instant::now();
    let placed = Placed::new(&model, &bvh, Transform::default());
    let table = asked.table();
    let profiles = Profiles::new(&table).context("a table of profiles is never empty")?;
    let points: Vec<SupportPoint> = needs
        .iter()
        .map(|need| SupportPoint {
            group: asked.group_of(need),
            ..SupportPoint::new(need.contact)
        })
        .collect();
    let trees = grow(&columns(&points, &placed, profiles), &placed, profiles);
    let supports = mesh_trees(&trees, &placed, profiles);
    let grow_s = started.elapsed().as_secs_f64();
    Ok(Supported {
        model,
        contacts: points.iter().map(|point| point.contact).collect(),
        groups: points.iter().map(|point| point.group).collect(),
        profiles: table,
        trees,
        supports,
        place_s,
        grow_s,
    })
}

/// `model` as it prints: its shell with a wall of `wall_mm`, cavity and infill counted
/// inside, and the outside of that wall a run keeps to; or the model itself.
fn printed(model: Mesh, wall_mm: Option<Scalar>) -> Result<(Mesh, Option<WallSide>)> {
    let Some(wall_mm) = wall_mm else {
        return Ok((model, None));
    };
    let bvh = Bvh::build(&model);
    let hollowed = hollow(
        &model,
        &bvh,
        &HollowSettings::for_wall(wall_mm),
        Cancel::never(),
    )
    .context("the model cannot be hollowed")?;
    let shell = hollowed.mesh;
    let side = WallSide {
        bvh: Arc::new(Bvh::build(&shell)),
        inner: hollowed.cavity.start..shell.faces.len().max(hollowed.cavity.end),
        into_model: Mat4::IDENTITY,
        inside: false,
    };
    Ok((shell, Some(side)))
}

/// Where `model` needs holding up and what each contact holds, as automatic placement
/// reads it off its own stack, kept to the outside of `side`'s wall.
fn needs_under(
    model: &Mesh,
    profile: &SupportProfile,
    sizing: Option<&AutoProfile>,
    side: Option<&WallSide>,
    layer_mm: Scalar,
) -> Result<Vec<Need>> {
    if layer_mm <= 0.0 {
        bail!("a layer of {layer_mm} mm cannot be cut");
    }
    let stack = PlaneSliceEngine
        .slice(
            model,
            &SliceSettings {
                layer_height: layer_mm,
                ..SliceSettings::default()
            },
        )
        .context("the model cannot be cut for placement")?;
    let lows = low_points(model, Transform::default());
    let blocked = Blocked::keeping(None, side.cloned());
    let mut needs = generate_supports(
        &stack,
        layer_mm,
        profile,
        sizing,
        &[],
        &lows,
        blocked.as_ref(),
        &mut |_| true,
    );
    if let Some(side) = side {
        needs.retain(|need| side.keeps(need.contact));
    }
    Ok(needs)
}

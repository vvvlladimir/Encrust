//! A model stood the way the corpus asks and supported as the window's Generate does,
//! keeping every tree so the bench can read what each carries.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use std::sync::Arc;

use core_engine::{Strengthened, SupportPlan, SupportShapes, Underside, strengthen};
use core_geometry::{
    Bvh, Mat4, Mesh, Quat, Scalar, Transform, Vec3, center_over_plate, drop_to_plate,
    lift_over_plate, orient_outward, transform_mesh, weld,
};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};
use core_supports::{
    Blocked, Need, Placed, Profiles, Reason, SupportPoint, SupportTree, WallSide, columns,
    generate_supports, grow, low_points, mesh_trees,
};
use core_volume::{Cancel, HollowSettings, hollow};
use encrust_cli::pipeline;
use printer_profiles::{BuildVolume, SupportProfile};

use crate::corpus::{Pose, Up};

/// `plan` with `share` of its density, for a sweep of how densely it places.
pub fn thinned(plan: &SupportPlan, share: f32) -> SupportPlan {
    let mut thinned = plan.clone();
    match &mut thinned.shapes {
        SupportShapes::One(profile) => profile.density *= share,
        SupportShapes::Sized { auto, .. } => auto.density *= share,
    }
    thinned
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
    /// Why placement asked for each of `contacts`; empty where a coupon set them by hand.
    pub reasons: Vec<Reason>,
    pub profiles: Vec<SupportProfile>,
    pub trees: Vec<SupportTree>,
    /// The supports as one mesh, plate coordinates.
    pub supports: Mesh,
    pub place_s: f64,
    pub grow_s: f64,
    /// What strengthening changed, unless `--no-strengthen` left it out.
    pub strengthened: Option<Strengthened>,
}

impl Supported {
    /// Grows the trees again over `contacts` as they stand now, each to its group.
    pub fn regrow(&mut self) -> Result<()> {
        let started = Instant::now();
        let bvh = Bvh::build(&self.model);
        let placed = Placed::new(&self.model, &bvh, Transform::default());
        let profiles =
            Profiles::new(&self.profiles).context("a table of profiles is never empty")?;
        let points: Vec<SupportPoint> = self
            .contacts
            .iter()
            .zip(&self.groups)
            .map(|(&contact, &group)| SupportPoint {
                group,
                ..SupportPoint::new(contact)
            })
            .collect();
        self.trees = grow(&columns(&points, &placed, profiles), &placed, profiles);
        self.supports = mesh_trees(&self.trees, &placed, profiles);
        self.grow_s += started.elapsed().as_secs_f64();
        Ok(())
    }

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

/// How a run sizes the tips placement asks for.
#[derive(Debug, Clone, Copy)]
pub struct Sizing {
    /// Strengthen tips under the wanted margin, as Generate does.
    pub strengthening: bool,
    /// Build every tip an automatic profile sizes to its lightest shape first, leaving its size
    /// to what strengthening finds it carries.
    pub lightest_first: bool,
}

/// `model` lifted by the placing profile's lift, hollowed to `wall_mm` where given, and supported
/// from outside as Generate does with nothing painted, blocked or placed by hand, sized as
/// `sizing` says.
pub fn supported(
    mut model: Mesh,
    asked: &SupportPlan,
    wall_mm: Option<Scalar>,
    (layer_mm, sizing): (Scalar, Sizing),
) -> Result<Supported> {
    let placing = asked.placing();
    let bounds = model.aabb().context("the model has no vertices")?;
    if bounds.mins.z < placing.lift_mm() {
        let offset = lift_over_plate(&bounds, placing.lift_mm());
        model = transform_mesh(&model, Transform::from_translation(offset));
    }
    let (model, side) = printed(model, wall_mm)?;
    let started = Instant::now();
    let (needs, reasons, strengthened) =
        needs_under(&model, asked, side.as_ref(), (layer_mm, sizing))?;
    let place_s = started.elapsed().as_secs_f64();

    let mut supported = Supported {
        model,
        contacts: needs.iter().map(|need| need.contact).collect(),
        groups: needs.iter().map(|need| asked.group_of(need)).collect(),
        reasons,
        profiles: asked.table(),
        trees: Vec::new(),
        supports: Mesh::default(),
        place_s,
        grow_s: 0.0,
        strengthened,
    };
    supported.regrow()?;
    Ok(supported)
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

/// Where `model` needs holding up, as automatic placement reads it off its own stack, kept to
/// the outside of `side`'s wall and sized as `sizing` says; why placement asked for each
/// contact, and what strengthening changed where asked.
fn needs_under(
    model: &Mesh,
    plan: &SupportPlan,
    side: Option<&WallSide>,
    (layer_mm, sizing): (Scalar, Sizing),
) -> Result<(Vec<Need>, Vec<Reason>, Option<Strengthened>)> {
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
        &plan.placing(),
        plan.sizing(),
        &[],
        &lows,
        blocked.as_ref(),
        &mut |_| true,
    );
    if let Some(side) = side {
        needs.retain(|need| side.keeps(need.contact));
    }
    let asked: Vec<Reason> = needs.iter().map(|need| need.reason).collect();
    if sizing.lightest_first {
        for need in &mut needs {
            need.reason = lightest(need.reason);
        }
    }
    if !sizing.strengthening {
        return Ok((needs, asked, None));
    }
    let underside = Underside {
        stack: &stack,
        layer_mm,
        surface: model,
        bvh: &Bvh::build(model),
        blocked: blocked.as_ref(),
    };
    let done = strengthen(underside, &mut needs, &[], plan, &mut || true)
        .context("the model cannot be read for what its tips carry")?;
    let added = needs.iter().skip(asked.len()).map(|need| need.reason);
    let reasons = asked.iter().copied().chain(added).collect();
    Ok((needs, reasons, Some(done)))
}

/// `reason` with nothing to hold, which an automatic profile builds to its lightest shape.
fn lightest(reason: Reason) -> Reason {
    match reason {
        Reason::Island { .. } => Reason::Island { area_mm2: 0.0 },
        Reason::Overhang { .. } => Reason::Overhang { carried_mm3: 0.0 },
        Reason::Step { .. } => Reason::Step { new_mm3: 0.0 },
        Reason::Steady => Reason::Steady,
    }
}

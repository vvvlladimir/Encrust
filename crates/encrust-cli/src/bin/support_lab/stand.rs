//! A model stood the way the corpus asks and supported the way the window's Generate
//! supports it, keeping every tree so the bench can read what each support carries.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use core_geometry::{
    Bvh, Mesh, Quat, Scalar, Transform, Vec3, center_over_plate, drop_to_plate, lift_over_plate,
    orient_outward, transform_mesh, weld,
};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};
use core_supports::{
    Placed, Profiles, SupportPoint, SupportTree, columns, generate_supports, grow, low_points,
    mesh_trees,
};
use encrust_cli::pipeline;
use printer_profiles::{BuildVolume, SupportProfile};

use crate::corpus::{Pose, Up};

/// A model on the plate and everything automatic placement stood under it.
pub struct Supported {
    /// The model, lifted and in plate coordinates.
    pub model: Mesh,
    /// Every contact placement asked for, standing or not.
    pub contacts: Vec<Vec3>,
    pub trees: Vec<SupportTree>,
    /// The supports as one mesh, plate coordinates.
    pub supports: Mesh,
    pub place_s: f64,
    pub grow_s: f64,
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

/// `model` lifted by the profile's lift and supported on its own, as Generate supports a
/// model on the plate with nothing painted, blocked or placed by hand.
pub fn supported(mut model: Mesh, profile: &SupportProfile, layer_mm: Scalar) -> Result<Supported> {
    let bounds = model.aabb().context("the model has no vertices")?;
    if bounds.mins.z < profile.lift_mm() {
        let offset = lift_over_plate(&bounds, profile.lift_mm());
        model = transform_mesh(&model, Transform::from_translation(offset));
    }
    let started = Instant::now();
    let contacts = contacts_under(&model, profile, layer_mm)?;
    let place_s = started.elapsed().as_secs_f64();

    let started = Instant::now();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let table = std::slice::from_ref(profile);
    let profiles = Profiles::new(table).context("a table of one profile is never empty")?;
    let points: Vec<SupportPoint> = contacts.iter().copied().map(SupportPoint::new).collect();
    let trees = grow(&columns(&points, &placed, profiles), &placed, profiles);
    let supports = mesh_trees(&trees, &placed, profiles);
    let grow_s = started.elapsed().as_secs_f64();
    Ok(Supported {
        model,
        contacts,
        trees,
        supports,
        place_s,
        grow_s,
    })
}

/// Where `model` needs holding up, as automatic placement reads it off its own stack.
fn contacts_under(model: &Mesh, profile: &SupportProfile, layer_mm: Scalar) -> Result<Vec<Vec3>> {
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
    Ok(generate_supports(
        &stack,
        layer_mm,
        profile,
        None,
        &[],
        &lows,
        None,
        &mut |_| true,
    )
    .into_iter()
    .map(|need| need.contact)
    .collect())
}

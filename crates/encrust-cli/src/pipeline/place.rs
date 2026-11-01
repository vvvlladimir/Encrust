use crate::args::ImportArgs;
use crate::profiles::Chosen;
use anyhow::{Context, Result};
use core_engine::{SupportPlan, Trial, orient_for_print};
use core_geometry::{
    Mesh, Quat, Scalar, Transform, Vec3, center_over_plate, drop_to_plate, glam::EulerRot,
    transform_mesh,
};
use core_plate::{OrientSettings, Printed, Watch};
use printer_profiles::{PrinterProfile, SupportProfile};

/// The turn auto-orientation applied, and why it chose it.
pub struct OrientSummary {
    pub rotation: Quat,
    pub peak_mm2: Scalar,
    pub overhang_mm2: Scalar,
    /// Whether the model needs no supports this way up, set straight on the plate.
    pub on_plate: bool,
}

impl OrientSummary {
    /// How far the turn goes, degrees.
    pub fn degrees(&self) -> Scalar {
        self.rotation.to_axis_angle().1.to_degrees()
    }

    /// The angles `--rotate X,Y,Z` repeats the turn with, degrees. Zero is added to each,
    /// so float dust never reads as a turn of minus nothing.
    pub fn rotate_degrees(&self) -> Vec3 {
        let (x, y, z) = self.rotation.to_euler(EulerRot::XYZEx);
        Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees()).round() + 0.0
    }

    /// The turn in words: its axis where that is one of the plate's own, and the flag that
    /// repeats it.
    pub(super) fn describe(&self) -> String {
        let flag = {
            let turn = self.rotate_degrees();
            format!("--rotate {:.0},{:.0},{:.0}", turn.x, turn.y, turn.z)
        };
        match self.axis_name() {
            Some(axis) => format!(
                "turned {:.0} degrees around {axis} ({flag})",
                self.degrees()
            ),
            None => format!("turned {:.0} degrees ({flag})", self.degrees()),
        }
    }

    /// X, Y or Z when the turn is about that axis of the plate, `None` for any other.
    pub(super) fn axis_name(&self) -> Option<&'static str> {
        let axis = self.rotation.to_axis_angle().0;
        [("X", Vec3::X), ("Y", Vec3::Y), ("Z", Vec3::Z)]
            .into_iter()
            .find(|(_, about)| axis.dot(*about).abs() > 1.0 - 1.0e-3)
            .map(|(name, _)| name)
    }
}

/// Applies the requested scale, orientation, rotation and placement, in that order: the
/// search judges the model at the size it prints.
pub(super) fn place(
    mesh: Mesh,
    args: &ImportArgs,
    supports: Option<&SupportPlan>,
    chosen: &Chosen,
) -> Result<(Mesh, Option<OrientSummary>)> {
    let transform = &args.transform;
    let mesh = match transform.scale {
        Some(scale) => transform_mesh(
            &mesh,
            Transform {
                scale,
                ..Transform::default()
            },
        ),
        None => mesh,
    };
    let (mesh, mut oriented) = if transform.orient {
        let summary = orientation(&mesh, args, supports, chosen)?;
        (turned_in_place(&mesh, summary.rotation)?, Some(summary))
    } else {
        (mesh, None)
    };

    let rotation = transform.rotate.map_or(Quat::IDENTITY, euler_degrees);
    let mut mesh = if rotation == Quat::IDENTITY {
        mesh
    } else {
        transform_mesh(
            &mesh,
            Transform {
                rotation,
                ..Transform::default()
            },
        )
    };
    // A --rotate that tips the model off the face --orient seated it on leaves it on supports.
    if let Some(summary) = oriented.as_mut()
        && !(rotation * Vec3::NEG_Z).abs_diff_eq(Vec3::NEG_Z, 1e-4)
    {
        summary.on_plate = false;
    }

    if transform.center {
        mesh = centred(mesh, chosen.printer.as_ref())?;
    } else if oriented.is_some() {
        // --rotate turns about the file's origin and can sink what --orient stood.
        mesh = dropped(mesh)?;
    }
    Ok((mesh, oriented))
}

/// The turn --orient finds, printed to choose between the search's best where a printer
/// and supports to print it on are known, as the window does.
pub(super) fn orientation(
    mesh: &Mesh,
    args: &ImportArgs,
    supports: Option<&SupportPlan>,
    chosen: &Chosen,
) -> Result<OrientSummary> {
    let transform = &args.transform;
    let profile = chosen.printer.as_ref();
    let settings = OrientSettings {
        preset: transform.orient_goal.into(),
        max_tilt_deg: transform.max_tilt,
        build_volume_mm: profile
            .map(|p| Vec3::new(p.build_volume.x, p.build_volume.y, p.build_volume.z)),
        ..orient_settings(supports.map(SupportPlan::placing).as_ref())
    };
    let trial = profile.zip(supports).map(|(printer, support)| Trial {
        printer,
        material: &chosen.material,
        support,
        raster_window: rayon::current_num_threads(),
    });
    let found = orient_for_print(
        &Printed::solid(mesh),
        &settings,
        trial.as_ref(),
        Watch::never(),
    )
    .context("cannot find an orientation for this model")?
    .into_iter()
    .next()
    .context("the search found no orientation for this model")?;
    Ok(OrientSummary {
        rotation: found.oriented.rotation,
        peak_mm2: found.oriented.score.peak_mm2.unwrap_or_default(),
        overhang_mm2: found.oriented.score.overhang_mm2,
        on_plate: supports.is_some() && found.oriented.on_plate,
    })
}

/// `mesh` turned by `rotation` and stood on z = 0 over the middle it stood over, where a
/// turn about the file's origin would carry it off, perhaps off the plate.
pub(super) fn turned_in_place(mesh: &Mesh, rotation: Quat) -> Result<Mesh> {
    let middle = |mesh: &Mesh| {
        mesh.aabb()
            .map(|bounds| ((bounds.mins + bounds.maxs) / 2.0).truncate())
            .context("mesh has no vertices")
    };
    let before = middle(mesh)?;
    let turned = transform_mesh(
        mesh,
        Transform {
            rotation,
            ..Transform::default()
        },
    );
    let back = (before - middle(&turned)?).extend(0.0);
    let moved = transform_mesh(&turned, Transform::from_translation(back));
    dropped(moved)
}

/// The model stood on z = 0, where it is in x and y.
pub(super) fn dropped(mesh: Mesh) -> Result<Mesh> {
    let bounds = mesh.aabb().context("mesh has no vertices")?;
    Ok(transform_mesh(
        &mesh,
        Transform::from_translation(drop_to_plate(&bounds)),
    ))
}

/// How the search judges a model that will stand on `supports`, or on the plate.
pub fn orient_settings(supports: Option<&SupportProfile>) -> OrientSettings {
    let plain = OrientSettings::default();
    supports.map_or(plain, |profile| OrientSettings {
        max_overhang_deg: profile.max_overhang_deg,
        contact_spacing_mm: profile.contact_spacing_mm,
        on_supports: true,
        lift_mm: profile.lift_mm(),
        rest_on_plate: true,
        ..plain
    })
}

/// The model stood in the middle of the plate and on z = 0.
/// Called again after relief, whose texture can sink the bottom under the plate, uncut.
pub(super) fn centred(mesh: Mesh, profile: Option<&PrinterProfile>) -> Result<Mesh> {
    let bounds = mesh.aabb().context("mesh has no vertices")?;
    let plate = profile.map_or(Vec3::ZERO, |p| {
        Vec3::new(p.build_volume.x, p.build_volume.y, 0.0)
    });
    let offset = drop_to_plate(&bounds) + center_over_plate(&bounds, plate.x, plate.y);
    if offset == Vec3::ZERO {
        return Ok(mesh);
    }
    Ok(transform_mesh(&mesh, Transform::from_translation(offset)))
}

pub(super) fn euler_degrees(angles: Vec3) -> Quat {
    let radians = angles * (std::f32::consts::PI / 180.0);
    Quat::from_rotation_z(radians.z)
        * Quat::from_rotation_y(radians.y)
        * Quat::from_rotation_x(radians.x)
}

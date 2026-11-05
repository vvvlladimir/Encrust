use std::fmt;
use std::ops::ControlFlow;
use std::path::PathBuf;

use anyhow::{Context, Result};
use core_geometry::{
    Adjacency, Bvh, DEFAULT_WELD_TOLERANCE, Mat4, Scalar, SurfacePath, SurfacePoint, Vec3,
    remove_duplicate_faces, surface_path, weld,
};
use serde::Serialize;

use crate::commands::GlobalArgs;
use crate::exit::Exit;
use crate::hollowing::point;
use crate::{json, pipeline};

#[derive(clap::Args, Debug)]
pub struct MeasureCommand {
    /// Mesh to measure on: `.stl`, `.obj` or `.3mf`, in the coordinates its file holds.
    pub input: PathBuf,

    /// One end, millimetres; the nearest point of the surface is taken.
    #[arg(long, value_name = "X,Y,Z", value_parser = point, allow_hyphen_values = true)]
    pub from: Vec3,

    /// The other end, taken the same way.
    #[arg(long, value_name = "X,Y,Z", value_parser = point, allow_hyphen_values = true)]
    pub to: Vec3,

    /// Also measure the shortest way between the ends over the surface, as the window's
    /// Measure along the surface does.
    #[arg(long)]
    pub along_surface: bool,
}

/// What `measure` prints: the ends as found on the surface and how far apart they are.
#[derive(Serialize)]
struct Measured {
    from: [Scalar; 3],
    to: [Scalar; 3],
    distance_mm: Scalar,
    delta_mm: [Scalar; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    along_surface_mm: Option<Scalar>,
    /// The walk the window draws, each edge it crosses: `--json` only.
    #[serde(skip_serializing_if = "Option::is_none")]
    along_surface_points: Option<Vec<[Scalar; 3]>>,
}

impl MeasureCommand {
    pub fn run(&self, global: &GlobalArgs) -> Result<Exit> {
        let loaded = pipeline::load(&self.input)?;
        // Repaired as import repairs it, so the walk crosses the faces the window's does.
        let mut mesh = weld(&loaded.mesh, DEFAULT_WELD_TOLERANCE).mesh;
        remove_duplicate_faces(&mut mesh);
        let bvh = Bvh::build(&mesh);
        let on = |near: Vec3| {
            let closest = bvh
                .closest(near)
                .with_context(|| format!("{} has no faces", self.input.display()))?;
            anyhow::Ok(SurfacePoint {
                face: closest.face,
                point: closest.point,
            })
        };
        let (from, to) = (on(self.from)?, on(self.to)?);
        let path = self
            .along_surface
            .then(|| {
                // Left to the shell's Ctrl-C, as `watches_stop` says.
                let mut go_on = |_| ControlFlow::Continue(());
                surface_path(
                    &mesh,
                    &Adjacency::of(&mesh),
                    Mat4::IDENTITY,
                    from,
                    to,
                    &mut go_on,
                )
            })
            .transpose()
            .context("cannot measure along the surface")?;
        let measured = Measured {
            from: from.point.to_array(),
            to: to.point.to_array(),
            distance_mm: from.point.distance(to.point),
            delta_mm: (to.point - from.point).to_array(),
            along_surface_mm: path.as_ref().map(SurfacePath::length_mm),
            along_surface_points: path
                .map(|path| path.points.iter().map(|point| point.to_array()).collect()),
        };
        if global.json {
            json::print(&measured)?;
        } else if !global.quiet {
            print!("{measured}");
        }
        Ok(Exit::Success)
    }
}

impl fmt::Display for Measured {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let point = |p: [Scalar; 3]| format!("{:.3}, {:.3}, {:.3} mm", p[0], p[1], p[2]);
        writeln!(f, "  {:<14}{}", "from", point(self.from))?;
        writeln!(f, "  {:<14}{}", "to", point(self.to))?;
        writeln!(f, "  {:<14}{:.3} mm", "distance", self.distance_mm)?;
        for (axis, delta) in ["dX", "dY", "dZ"].iter().zip(self.delta_mm) {
            writeln!(f, "  {axis:<14}{delta:.3} mm")?;
        }
        if let Some(along_mm) = self.along_surface_mm {
            writeln!(f, "  {:<14}{along_mm:.3} mm", "along surface")?;
        }
        Ok(())
    }
}

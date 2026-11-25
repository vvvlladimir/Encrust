//! The objects of a saved plate as `inspect` lists them: the ids `--only` takes, and what each
//! is and where it stands (ADR 0479).

use std::fmt;

use core_engine::project::Project;
use core_geometry::transform_mesh;
use serde::Serialize;

/// One object of a project.
#[derive(Debug, Serialize)]
pub struct PlateObject {
    pub id: u64,
    pub name: String,
    /// The plate it stands on, counted from one.
    pub plate: u32,
    /// Its box on the plate, mm.
    pub min_mm: [f32; 3],
    pub max_mm: [f32; 3],
    pub hollow: bool,
    /// Support trees standing under it.
    pub supports: usize,
    /// The id of the model it is a plug for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plug_of: Option<u64>,
}

/// Every object of `project`, in plate order.
pub fn plate_objects(project: &Project) -> Vec<PlateObject> {
    let objects = &project.manifest.objects;
    (objects.iter().zip(&project.models))
        .map(|(object, meshes)| {
            let placed = transform_mesh(&meshes.source, object.transform);
            let (min_mm, max_mm) = placed.aabb().map_or(([0.0; 3], [0.0; 3]), |bounds| {
                (bounds.mins.to_array(), bounds.maxs.to_array())
            });
            let held = &object.supports;
            PlateObject {
                id: object.id.0,
                name: object.name.clone(),
                plate: object.plate + 1,
                min_mm,
                max_mm,
                hollow: meshes.shell.is_some(),
                supports: held.frozen.len() + held.grown.len(),
                plug_of: (object.plug_of)
                    .and_then(|of| objects.get(of as usize))
                    .map(|holder| holder.id.0),
            }
        })
        .collect()
}

impl fmt::Display for PlateObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [x0, y0, z0] = self.min_mm;
        let [x1, y1, z1] = self.max_mm;
        write!(
            f,
            "  {:<4} {:<28} plate {}  {x0:.1},{y0:.1},{z0:.1} .. {x1:.1},{y1:.1},{z1:.1} mm",
            self.id, self.name, self.plate
        )?;
        if self.hollow {
            write!(f, ", hollow")?;
        }
        if self.supports > 0 {
            write!(f, ", {} support trees", self.supports)?;
        }
        if let Some(of) = self.plug_of {
            write!(f, ", plug of {of}")?;
        }
        writeln!(f)
    }
}

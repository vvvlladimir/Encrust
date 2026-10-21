//! Meshes written out to be opened beside each other: the supported model, and a marker
//! at every place the bench names, one file per family so each can be coloured apart.

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use core_geometry::{Mesh, Vec3};

use core_analysis::RiskKind;

use crate::cure::Cured;
use crate::shape::{Finding, Kind};

/// Half the width of a marker, millimetres: big enough to see on a figure 30 mm tall.
const MARKER_MM: f32 = 0.6;

/// Writes the model with its supports, and its markers, as `<stem>*.stl` in `dir`.
pub fn write(
    dir: &Path,
    stem: &str,
    model: &Mesh,
    supports: &Mesh,
    findings: &[Finding],
    cured: &Cured,
    neck_mpa: f32,
) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("cannot make {}", dir.display()))?;
    let mut whole = model.clone();
    append(&mut whole, supports);
    stl(&dir.join(format!("{stem}.stl")), &whole)?;

    let falls: Vec<Vec3> = findings
        .iter()
        .filter(|f| f.kind == Kind::Hanging)
        .map(|f| Vec3::from(f.at))
        .chain(
            cured
                .risks
                .iter()
                .filter(|r| fails(&r.risk.kind, neck_mpa))
                .map(|r| r.at.extend(r.z_mm)),
        )
        .collect();
    let marks = at(findings, &[Kind::Visible, Kind::ThinTip, Kind::OnModel]);
    let waste = at(
        findings,
        &[Kind::Bare, Kind::Overloaded, Kind::Crowded, Kind::Slender],
    );
    for (family, points) in [("falls", falls), ("marks", marks), ("balance", waste)] {
        if !points.is_empty() {
            stl(&dir.join(format!("{stem}-{family}.stl")), &markers(&points))?;
        }
    }
    Ok(())
}

/// What leaves a part in the vat: cured over nothing or torn off what held it.
fn fails(kind: &RiskKind, neck_mpa: f32) -> bool {
    match kind {
        RiskKind::Island { .. } | RiskKind::Floating { .. } => true,
        RiskKind::Lever { stress_mpa, .. } => *stress_mpa >= neck_mpa,
        _ => false,
    }
}

fn at(findings: &[Finding], kinds: &[Kind]) -> Vec<Vec3> {
    findings
        .iter()
        .filter(|f| kinds.contains(&f.kind))
        .map(|f| Vec3::from(f.at))
        .collect()
}

/// An octahedron at each point.
fn markers(points: &[Vec3]) -> Mesh {
    let mut mesh = Mesh::default();
    for point in points {
        let offset = mesh.vertices.len() as u32;
        mesh.vertices.extend(
            [
                Vec3::X,
                Vec3::NEG_X,
                Vec3::Y,
                Vec3::NEG_Y,
                Vec3::Z,
                Vec3::NEG_Z,
            ]
            .map(|axis| *point + axis * MARKER_MM),
        );
        let faces = [
            [0, 2, 4],
            [2, 1, 4],
            [1, 3, 4],
            [3, 0, 4],
            [2, 0, 5],
            [1, 2, 5],
            [3, 1, 5],
            [0, 3, 5],
        ];
        mesh.faces
            .extend(faces.map(|[a, b, c]| [a + offset, b + offset, c + offset]));
    }
    mesh
}

pub fn append(mesh: &mut Mesh, other: &Mesh) {
    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&other.vertices);
    mesh.faces.extend(
        other
            .faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
}

/// Binary STL, which every tool reads.
pub fn stl(path: &Path, mesh: &Mesh) -> Result<()> {
    let mut bytes = Vec::with_capacity(84 + 50 * mesh.faces.len());
    bytes.extend_from_slice(&[0u8; 80]);
    bytes.extend_from_slice(&(mesh.faces.len() as u32).to_le_bytes());
    for triangle in mesh.triangles() {
        let normal = triangle.normal_unnormalized().normalize_or_zero();
        for v in [normal, triangle.a, triangle.b, triangle.c] {
            for c in v.to_array() {
                bytes.extend_from_slice(&c.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&[0, 0]);
    }
    std::fs::File::create(path)
        .and_then(|mut file| file.write_all(&bytes))
        .with_context(|| format!("cannot write {}", path.display()))
}

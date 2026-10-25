//! A print answering what the bench can only assume: what tears a tip off and what it leaves.
//! A spec file lays it out; 

use std::path::Path;

use anyhow::{Context, Result};
use core_geometry::{Bvh, Mesh, Quat, Transform, Vec2, Vec3, ball, cuboid, oriented_box};
use core_supports::{Placed, Profiles, SupportPoint, columns, grow, mesh_trees};
use printer_profiles::SupportProfile;
use serde::Deserialize;

use crate::export::append;
use crate::stand::Supported;

/// Each plate, millimetres: wide enough to pull, thin enough to bend no tip by its weight.
const PLATE_MM: f32 = 8.0;
const PLATE_THICK_MM: f32 = 1.2;
const BALL_MM: f32 = 3.0;
/// Between specimens of a block, millimetres: clear of the widest pads.
const PITCH_MM: f32 = 14.0;
/// From a block's last column to the next one's first: room for the next block's ruler.
const BLOCK_GAP_MM: f32 = 24.0;
/// Where the lowest point of every specimen stands, millimetres.
const LIFT_MM: f32 = 5.0;
/// How far up its slope from the low edge a leant plate's first row of tips stands,
/// millimetres: on the edge, so the first layer it cures is already held.
const EDGE_INSET_MM: f32 = 0.1;
const RULER_OFFSET_MM: f32 = 10.0;
const RULER_WIDTH_MM: f32 = 2.5;
const RULER_MM: f32 = 1.0;
const BUMP_MM: f32 = 1.0;
const BUMP_PITCH_MM: f32 = 1.6;
/// A block's number stands past its corner in bumps twice as tall as a row's.
const BLOCK_BUMP_MM: f32 = 2.0;

/// A coupon as its spec file states it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    #[serde(rename = "shape")]
    pub shapes: Vec<TipShape>,
    #[serde(rename = "block")]
    pub blocks: Vec<Block>,
}

/// One way of making a tip: a row of every block that names it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TipShape {
    pub name: String,
    /// The contact's own width, millimetres.
    pub tip_mm: f32,
    /// How wide the cone is where it meets the pillar, and the pillar's width, millimetres.
    pub join_mm: f32,
    /// How long the cone is, millimetres.
    pub cone_mm: f32,
    /// How far the tip is moved into the part from a contact a layer under its surface,
    /// millimetres: one layer is just touching.
    pub bite_mm: f32,
}

/// One block: one body, a row for each shape it names and a column for each count of tips.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    pub name: String,
    pub body: Body,
    /// How far a leant plate leans, degrees.
    #[serde(default)]
    pub tilt_deg: f32,
    /// The shapes it is printed in, by name; every shape of the spec when empty.
    #[serde(default)]
    pub shapes: Vec<String>,
    pub holders: Vec<usize>,
}

/// What one specimen is.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Body {
    /// A plate square to the film: all of its first layer appears at once.
    Flat,
    /// A plate leant by its block's `tilt_deg`: it grows a sliver at a time, as a figure does.
    Leant,
    /// A ball on one tip: a test of the tip and its bite with next to no pull.
    Ball,
}

impl Spec {
    pub fn read(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let spec: Self = toml::from_str(&text)
            .with_context(|| format!("{} is not a coupon spec", path.display()))?;
        for block in &spec.blocks {
            for name in &block.shapes {
                anyhow::ensure!(
                    spec.shapes.iter().any(|shape| &shape.name == name),
                    "block {} names a shape {name} the spec does not define",
                    block.name
                );
            }
        }
        Ok(spec)
    }

    /// The shapes `block` is printed in, in its rows' order.
    pub fn rows<'a>(&'a self, block: &'a Block) -> Vec<&'a TipShape> {
        if block.shapes.is_empty() {
            return self.shapes.iter().collect();
        }
        block
            .shapes
            .iter()
            .filter_map(|name| self.shapes.iter().find(|shape| &shape.name == name))
            .collect()
    }
}

/// One specimen and where it stands, plate millimetres.
pub struct Specimen {
    pub block: usize,
    pub row: usize,
    pub column: usize,
    pub at: Vec2,
}

/// Where every specimen of `spec` stands with the coupon centred on `centre`.
pub fn layout(spec: &Spec, centre: Vec2) -> Vec<Specimen> {
    let span = |count: usize| PITCH_MM * (count as f32 - 1.0);
    let width: f32 = spec
        .blocks
        .iter()
        .map(|block| span(block.holders.len()))
        .sum::<f32>()
        + BLOCK_GAP_MM * (spec.blocks.len() as f32 - 1.0);
    let tallest = spec
        .blocks
        .iter()
        .map(|block| spec.rows(block).len())
        .max()
        .unwrap_or(1);
    let mut x = centre.x - width / 2.0;
    let y = centre.y - span(tallest) / 2.0;
    let mut specimens = Vec::new();
    for (index, block) in spec.blocks.iter().enumerate() {
        for row in 0..spec.rows(block).len() {
            for column in 0..block.holders.len() {
                let at = Vec2::new(x + column as f32 * PITCH_MM, y + row as f32 * PITCH_MM);
                specimens.push(Specimen {
                    block: index,
                    row,
                    column,
                    at,
                });
            }
        }
        x += span(block.holders.len()) + BLOCK_GAP_MM;
    }
    specimens
}

/// The specimen standing over `at`, if one is within half a pitch.
pub fn specimen_at(specimens: &[Specimen], at: Vec2) -> Option<&Specimen> {
    specimens
        .iter()
        .find(|specimen| specimen.at.distance(at) < PITCH_MM / 2.0)
}

/// One specimen built: its body on its supports, and the profile they were built to.
pub struct Built {
    pub supported: Supported,
    pub profile: SupportProfile,
}

/// Every specimen of `spec` centred on `centre`, built on `base` with only the tip's shape
/// changed and nothing merged or braced, in `layout` order.
pub fn build(
    spec: &Spec,
    base: &SupportProfile,
    layer_mm: f32,
    centre: Vec2,
) -> Result<Vec<Built>> {
    layout(spec, centre)
        .iter()
        .map(|specimen| {
            let block = &spec.blocks[specimen.block];
            let shape = spec.rows(block)[specimen.row];
            let profile = profile_for(base, shape);
            let holders = block.holders[specimen.column];
            let (body, points) = body(block, specimen.at, holders, layer_mm);
            let supported = supported(&profile, body, points)
                .with_context(|| format!("block {}, shape {}", block.name, shape.name))?;
            Ok(Built { supported, profile })
        })
        .collect()
}

/// The coupon as one mesh: every specimen on its supports and every block's rulers.
pub fn coupon(spec: &Spec, built: &[Built], centre: Vec2) -> Result<Mesh> {
    let specimens = layout(spec, centre);
    let mut whole = Mesh::default();
    for built in built {
        append(&mut whole, &built.supported.model);
        append(&mut whole, &built.supported.supports);
    }
    for (index, block) in spec.blocks.iter().enumerate() {
        let first = specimens
            .iter()
            .find(|specimen| specimen.block == index)
            .context("every block has a specimen")?;
        let size = (spec.rows(block).len(), block.holders.len());
        append(&mut whole, &rulers(first.at, size, index + 1));
    }
    Ok(whole)
}

/// `base` with its tip made to `shape`, and nothing that would tie two specimens together.
fn profile_for(base: &SupportProfile, shape: &TipShape) -> SupportProfile {
    let mut profile = base.clone();
    profile.branching.enabled = false;
    profile.bracing.enabled = false;
    profile.raft.enabled = false;
    profile.tip.contact_diameter_mm = shape.tip_mm;
    profile.tip.contact_depth_mm = shape.bite_mm;
    profile.top.length_mm = shape.cone_mm;
    profile.middle.diameter_mm = shape.join_mm;
    profile.bottom.lower_diameter_mm = profile.bottom.lower_diameter_mm.max(shape.join_mm);
    profile
}

/// The body of one specimen over `at`, and the contacts under it: each a layer off the
/// surface it holds, where placement puts one.
fn body(block: &Block, at: Vec2, holders: usize, layer_mm: f32) -> (Mesh, Vec<Vec3>) {
    let half = PLATE_MM / 2.0;
    let thick = PLATE_THICK_MM / 2.0;
    match block.body {
        Body::Flat => {
            let low = Vec3::new(at.x - half, at.y - half, LIFT_MM);
            let plate = cuboid(low, low + Vec3::new(PLATE_MM, PLATE_MM, PLATE_THICK_MM));
            let points = spots(holders)
                .into_iter()
                .map(|(u, v)| Vec3::new(at.x + u, at.y + v, LIFT_MM - layer_mm))
                .collect();
            (plate, points)
        }
        Body::Leant => {
            let turn = Quat::from_rotation_x(block.tilt_deg.to_radians());
            let (across, along, up) = (turn * Vec3::X, turn * Vec3::Y, turn * Vec3::Z);
            let rise = half * along.z.abs() + thick * up.z.abs();
            let centre = Vec3::new(at.x, at.y, LIFT_MM + rise);
            let plate = oriented_box(centre, [across * half, along * half, up * thick]);
            let underside = centre - up * thick;
            let points = leant_spots(holders)
                .into_iter()
                .map(|(u, v)| underside + across * u + along * v - up * layer_mm)
                .collect();
            (plate, points)
        }
        Body::Ball => {
            let radius = BALL_MM / 2.0;
            let sphere = ball(Vec3::new(at.x, at.y, LIFT_MM + radius), radius, 24, 32);
            (sphere, vec![Vec3::new(at.x, at.y, LIFT_MM - layer_mm)])
        }
    }
}

/// `body` with columns stood under `points` to `profile`.
fn supported(profile: &SupportProfile, body: Mesh, points: Vec<Vec3>) -> Result<Supported> {
    let tips: Vec<SupportPoint> = points.iter().copied().map(SupportPoint::new).collect();
    let bvh = Bvh::build(&body);
    let placed = Placed::new(&body, &bvh, Transform::default());
    let table = std::slice::from_ref(profile);
    let profiles = Profiles::new(table).context("a table of one profile is never empty")?;
    let trees = grow(&columns(&tips, &placed, profiles), &placed, profiles);
    let standing: usize = trees.iter().map(|tree| tree.tip_count()).sum();
    anyhow::ensure!(
        standing == tips.len(),
        "only {standing} of {} tips found room under a specimen",
        tips.len()
    );
    let supports = mesh_trees(&trees, &placed, profiles);
    Ok(Supported {
        model: body,
        groups: vec![0; points.len()],
        contacts: points,
        profiles: vec![profile.clone()],
        trees,
        supports,
        place_s: 0.0,
        grow_s: 0.0,
    })
}

/// Where `holders` tips go under a plate, millimetres off its middle: one, a diagonal pair,
/// or a square grid as wide as the plate allows.
fn spots(holders: usize) -> Vec<(f32, f32)> {
    match holders {
        1 => vec![(0.0, 0.0)],
        2 => vec![(-2.0, -2.0), (2.0, 2.0)],
        _ => {
            let side = (holders as f32).sqrt().round() as usize;
            let reach = 3.2_f32.min(1.25 * (side as f32 - 1.0));
            let step = 2.0 * reach / (side as f32 - 1.0).max(1.0);
            (0..side * side)
                .map(|at| {
                    let (u, v) = (at % side, at / side);
                    (u as f32 * step - reach, v as f32 * step - reach)
                })
                .collect()
        }
    }
}

/// Where `holders` tips go under a leant plate, millimetres across it and up its slope
/// from its middle: the first row on the low edge, which cures first, the rest above it.
fn leant_spots(holders: usize) -> Vec<(f32, f32)> {
    let edge = -PLATE_MM / 2.0 + EDGE_INSET_MM;
    match holders {
        1 => vec![(0.0, edge)],
        2 => vec![(-2.5, edge), (2.5, edge)],
        4 => vec![(-2.5, edge), (2.5, edge), (-2.5, 1.5), (2.5, 1.5)],
        _ => {
            let side = (holders as f32).sqrt().round().max(2.0) as usize;
            let step = (PLATE_MM / 2.0 - 1.0 - edge) / (side as f32 - 1.0);
            let across = 2.0 * 2.8 / (side as f32 - 1.0);
            (0..side * side)
                .map(|at| {
                    let (u, v) = (at % side, at / side);
                    (u as f32 * across - 2.8, edge + v as f32 * step)
                })
                .collect()
        }
    }
}

/// A block's two rails cured flat on the plate, where nothing tears them off: numbered bumps
/// beside each row and column, so each specimen is tested alike.
fn rulers(first: Vec2, (rows, columns): (usize, usize), number: usize) -> Mesh {
    let mut mesh = Mesh::default();
    let left = first.x - RULER_OFFSET_MM;
    let near = first.y - RULER_OFFSET_MM;
    let half = RULER_WIDTH_MM / 2.0;
    let far_y = first.y + PITCH_MM * (rows as f32 - 0.5);
    let far_x = first.x + PITCH_MM * (columns as f32 - 0.5);
    let tail = near - half - BUMP_PITCH_MM * (number as f32 + 0.5);
    let rails = [
        cuboid(
            Vec3::new(left - half, tail, 0.0),
            Vec3::new(left + half, far_y, RULER_MM),
        ),
        cuboid(
            Vec3::new(left - half, near - half, 0.0),
            Vec3::new(far_x, near + half, RULER_MM),
        ),
    ];
    for rail in &rails {
        append(&mut mesh, rail);
    }
    for row in 0..rows {
        for at in bumps(row + 1, first.y + row as f32 * PITCH_MM) {
            append(&mut mesh, &bump(Vec2::new(left, at), BUMP_MM));
        }
    }
    for column in 0..columns {
        for at in bumps(column + 1, first.x + column as f32 * PITCH_MM) {
            append(&mut mesh, &bump(Vec2::new(at, near), BUMP_MM));
        }
    }
    for at in 0..number {
        let y = near - half - BUMP_PITCH_MM * (at as f32 + 0.5);
        append(&mut mesh, &bump(Vec2::new(left, y), BLOCK_BUMP_MM));
    }
    mesh
}

/// Where `count` bumps sit, centred on `middle` along the rail.
fn bumps(count: usize, middle: f32) -> impl Iterator<Item = f32> {
    let first = middle - BUMP_PITCH_MM * (count as f32 - 1.0) / 2.0;
    (0..count).map(move |at| first + at as f32 * BUMP_PITCH_MM)
}

fn bump(at: Vec2, tall_mm: f32) -> Mesh {
    let half = BUMP_MM / 2.0;
    cuboid(
        Vec3::new(at.x - half, at.y - half, RULER_MM - 0.1),
        Vec3::new(at.x + half, at.y + half, RULER_MM + tall_mm),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = r#"
        [[shape]]
        name = "narrow"
        tip_mm = 0.3
        join_mm = 0.7
        cone_mm = 1.5
        bite_mm = 0.2

        [[shape]]
        name = "wide"
        tip_mm = 0.3
        join_mm = 1.2
        cone_mm = 2.0
        bite_mm = 0.05

        [[block]]
        name = "flat"
        body = "flat"
        holders = [9, 16]

        [[block]]
        name = "leant"
        body = "leant"
        tilt_deg = 30
        shapes = ["wide"]
        holders = [4]
    "#;

    #[test]
    fn a_block_with_no_shapes_is_printed_in_every_one() {
        let spec: Spec = toml::from_str(SPEC).expect("the spec is well formed");
        assert_eq!(spec.rows(&spec.blocks[0]).len(), 2);
        assert_eq!(spec.rows(&spec.blocks[1])[0].name, "wide");
        assert_eq!(layout(&spec, Vec2::ZERO).len(), 2 * 2 + 1);
    }

    #[test]
    fn a_grid_of_tips_stays_under_the_plate() {
        for holders in [4, 9, 16, 25] {
            let spots = spots(holders);
            assert_eq!(spots.len(), holders);
            assert!(
                spots
                    .iter()
                    .all(|(u, v)| u.abs() < PLATE_MM / 2.0 && v.abs() < PLATE_MM / 2.0),
                "{holders} tips must all stand under an 8 mm plate"
            );
        }
    }

    #[test]
    fn a_leant_plate_is_held_first_along_its_low_edge() {
        for holders in [1, 2, 4, 9] {
            let spots = leant_spots(holders);
            assert_eq!(spots.len(), holders);
            assert!(
                spots
                    .iter()
                    .any(|(_, v)| *v <= -PLATE_MM / 2.0 + EDGE_INSET_MM),
                "{holders} tips must hold the edge that cures first"
            );
        }
    }

    #[test]
    fn every_specimen_of_the_coupon_has_its_own_place() {
        let spec: Spec = toml::from_str(SPEC).expect("the spec is well formed");
        let specimens = layout(&spec, Vec2::new(109.0, 61.0));
        for specimen in &specimens {
            let found = specimen_at(&specimens, specimen.at).expect("a specimen finds itself");
            assert!(
                std::ptr::eq(found, specimen),
                "two specimens closer than half a pitch"
            );
        }
    }
}

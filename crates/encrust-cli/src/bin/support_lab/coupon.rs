//! A print that answers what the bench can only assume: what tears a tip off a part. Four
//! blocks tell the causes apart — flat plates, the worst suction there is; the same plates
//! leant as a figure is; balls on one tip, which pull on the film almost not at all; and
//! one plate bitten to three depths. local-docs/supports/README.md says how to read it.

use anyhow::{Context, Result};
use core_geometry::{Bvh, Mesh, Quat, Transform, Vec2, Vec3, ball, cuboid, oriented_box};
use core_supports::{Placed, Profiles, SupportPoint, columns, grow, mesh_trees};
use printer_profiles::SupportProfile;

use crate::export::append;

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
const TILT_DEG: f32 = 45.0;
/// How far up its slope from the low edge a leant plate's first row of tips stands,
/// millimetres: on the edge, so the first layer it cures is already held.
const EDGE_INSET_MM: f32 = 0.1;
/// The bite the `bite` block's tips are 0.4 mm wide at, as a figure's medium tips are.
const BITE_TIP_MM: f32 = 0.4;
const RULER_OFFSET_MM: f32 = 10.0;
const RULER_WIDTH_MM: f32 = 2.5;
const RULER_MM: f32 = 1.0;
const BUMP_MM: f32 = 1.0;
const BUMP_PITCH_MM: f32 = 1.6;
/// A block's number stands past its corner in bumps twice as tall as a row's.
const BLOCK_BUMP_MM: f32 = 2.0;

/// What one specimen is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Body {
    /// A plate square to the film: all of its first layer appears at once.
    Flat,
    /// A plate leant `TILT_DEG`: its layers grow a sliver at a time, as a figure's do.
    Leant,
    /// A ball on one tip: a test of the tip and its bite with next to no pull.
    Ball,
}

/// One block of the coupon: one body, rows and columns of what changes.
pub struct Block {
    pub name: &'static str,
    pub body: Body,
    pub rows: Vec<Setting>,
    pub columns: Vec<Setting>,
}

/// What a row or a column sets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Setting {
    TipMm(f32),
    Holders(usize),
    DepthMm(f32),
}

impl Setting {
    pub fn label(self) -> String {
        match self {
            Self::TipMm(mm) => format!("tip {mm} mm"),
            Self::Holders(n) => format!("{n} tips"),
            Self::DepthMm(mm) => format!("bite {mm} mm"),
        }
    }
}

/// The four blocks, left to right.
pub fn blocks() -> Vec<Block> {
    use Setting::{DepthMm, Holders, TipMm};
    vec![
        Block {
            name: "flat",
            body: Body::Flat,
            rows: vec![TipMm(0.3), TipMm(0.4), TipMm(0.5)],
            columns: vec![Holders(9), Holders(16), Holders(25)],
        },
        Block {
            name: "leant",
            body: Body::Leant,
            rows: vec![TipMm(0.2), TipMm(0.3), TipMm(0.4)],
            columns: vec![Holders(2), Holders(4), Holders(9)],
        },
        Block {
            name: "ball",
            body: Body::Ball,
            rows: vec![
                TipMm(0.15),
                TipMm(0.2),
                TipMm(0.25),
                TipMm(0.3),
                TipMm(0.4),
                TipMm(0.5),
            ],
            columns: vec![Holders(1)],
        },
        Block {
            name: "bite",
            body: Body::Flat,
            rows: vec![DepthMm(0.05), DepthMm(0.2), DepthMm(0.4)],
            columns: vec![Holders(9)],
        },
    ]
}

/// One specimen and where it stands, plate millimetres.
pub struct Specimen {
    pub block: usize,
    pub row: usize,
    pub column: usize,
    pub at: Vec2,
}

/// Where every specimen of `blocks` stands with the coupon centred on `centre`.
pub fn layout(blocks: &[Block], centre: Vec2) -> Vec<Specimen> {
    let span = |count: usize| PITCH_MM * (count as f32 - 1.0);
    let width: f32 = blocks
        .iter()
        .map(|block| span(block.columns.len()))
        .sum::<f32>()
        + BLOCK_GAP_MM * (blocks.len() as f32 - 1.0);
    let tallest = blocks
        .iter()
        .map(|block| block.rows.len())
        .max()
        .unwrap_or(1);
    let mut x = centre.x - width / 2.0;
    let y = centre.y - span(tallest) / 2.0;
    let mut specimens = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        for row in 0..block.rows.len() {
            for column in 0..block.columns.len() {
                let at = Vec2::new(x + column as f32 * PITCH_MM, y + row as f32 * PITCH_MM);
                specimens.push(Specimen {
                    block: index,
                    row,
                    column,
                    at,
                });
            }
        }
        x += span(block.columns.len()) + BLOCK_GAP_MM;
    }
    specimens
}

/// The specimen standing over `at`, if one is within half a pitch.
pub fn specimen_at(specimens: &[Specimen], at: Vec2) -> Option<&Specimen> {
    specimens
        .iter()
        .find(|specimen| specimen.at.distance(at) < PITCH_MM / 2.0)
}

/// The coupon as one mesh: every specimen on its supports and every block's rulers, built
/// on `base` with only what a row or a column sets changed and nothing merged or braced.
pub fn coupon(base: &SupportProfile, layer_mm: f32, centre: Vec2) -> Result<Mesh> {
    let blocks = blocks();
    let specimens = layout(&blocks, centre);
    let mut whole = Mesh::default();
    for specimen in &specimens {
        let block = &blocks[specimen.block];
        let (profile, holders) = setup(base, block, specimen);
        let (body, points) = body(block.body, specimen.at, holders, layer_mm);
        append(&mut whole, &supported(&profile, &body, &points)?);
    }
    for (index, block) in blocks.iter().enumerate() {
        let first = specimens
            .iter()
            .find(|specimen| specimen.block == index)
            .context("every block has a specimen")?;
        append(&mut whole, &rulers(first.at, block, index + 1));
    }
    Ok(whole)
}

/// The profile one specimen is supported to and how many tips hold it.
fn setup(base: &SupportProfile, block: &Block, specimen: &Specimen) -> (SupportProfile, usize) {
    let mut profile = base.clone();
    profile.branching.enabled = false;
    profile.bracing.enabled = false;
    profile.raft.enabled = false;
    let mut holders = 1;
    let mut tip_mm = if block.name == "bite" {
        BITE_TIP_MM
    } else {
        profile.tip.contact_diameter_mm
    };
    for setting in [block.rows[specimen.row], block.columns[specimen.column]] {
        match setting {
            Setting::TipMm(mm) => tip_mm = mm,
            Setting::Holders(n) => holders = n,
            Setting::DepthMm(mm) => profile.tip.contact_depth_mm = mm,
        }
    }
    profile.tip.contact_diameter_mm = tip_mm;
    profile.middle.diameter_mm = profile.middle.diameter_mm.max(tip_mm * 2.0);
    (profile, holders)
}

/// The body of one specimen over `at`, and the contacts under it: each a layer off the
/// surface it holds, where placement puts one.
fn body(kind: Body, at: Vec2, holders: usize, layer_mm: f32) -> (Mesh, Vec<Vec3>) {
    let half = PLATE_MM / 2.0;
    let thick = PLATE_THICK_MM / 2.0;
    match kind {
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
            let turn = Quat::from_rotation_x(TILT_DEG.to_radians());
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
fn supported(profile: &SupportProfile, body: &Mesh, points: &[Vec3]) -> Result<Mesh> {
    let points: Vec<SupportPoint> = points.iter().copied().map(SupportPoint::new).collect();
    let bvh = Bvh::build(body);
    let placed = Placed::new(body, &bvh, Transform::default());
    let table = std::slice::from_ref(profile);
    let profiles = Profiles::new(table).context("a table of one profile is never empty")?;
    let trees = grow(&columns(&points, &placed, profiles), &placed, profiles);
    let standing: usize = trees.iter().map(|tree| tree.tip_count()).sum();
    anyhow::ensure!(
        standing == points.len(),
        "only {standing} of {} tips found room under a specimen",
        points.len()
    );
    let mut mesh = body.clone();
    append(&mut mesh, &mesh_trees(&trees, &placed, profiles));
    Ok(mesh)
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

/// A block's two rails cured flat on the plate, where nothing tears them off: beside each
/// row as many bumps as its place, beside each column the same, and past the corner the
/// block's own number in taller bumps. They mark no specimen, so each is tested alike.
fn rulers(first: Vec2, block: &Block, number: usize) -> Mesh {
    let mut mesh = Mesh::default();
    let left = first.x - RULER_OFFSET_MM;
    let near = first.y - RULER_OFFSET_MM;
    let half = RULER_WIDTH_MM / 2.0;
    let far_y = first.y + PITCH_MM * (block.rows.len() as f32 - 0.5);
    let far_x = first.x + PITCH_MM * (block.columns.len() as f32 - 0.5);
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
    for row in 0..block.rows.len() {
        for at in bumps(row + 1, first.y + row as f32 * PITCH_MM) {
            append(&mut mesh, &bump(Vec2::new(left, at), BUMP_MM));
        }
    }
    for column in 0..block.columns.len() {
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
        let specimens = layout(&blocks(), Vec2::new(109.0, 61.0));
        for specimen in &specimens {
            let found = specimen_at(&specimens, specimen.at).expect("a specimen finds itself");
            assert!(
                std::ptr::eq(found, specimen),
                "two specimens closer than half a pitch"
            );
        }
    }
}

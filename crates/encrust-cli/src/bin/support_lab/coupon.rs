//! A print that answers what the bench can only assume: how much a tip of each width
//! holds before the peel tears it off, and what mark it leaves. Rows are tip widths,
//! columns the tips under one plate; local-docs/supports/README.md says how to read it.

use anyhow::{Context, Result};
use core_geometry::{Bvh, Mesh, Transform, Vec2, Vec3, cuboid};
use core_supports::{Placed, Profiles, SupportPoint, columns, grow, mesh_trees};
use printer_profiles::SupportProfile;

use crate::export::append;

/// Tip widths, one row each, millimetres.
pub const TIPS_MM: [f32; 6] = [0.15, 0.2, 0.25, 0.3, 0.4, 0.5];
/// How many tips hold each plate, one column each: 64, 32, 16 and about 7 mm² a tip.
pub const HOLDERS: [usize; 4] = [1, 2, 4, 9];
/// Each plate, millimetres: wide enough to pull, thin enough to bend no tip by its weight.
const PLATE_MM: f32 = 8.0;
const PLATE_THICK_MM: f32 = 1.2;
const PITCH_MM: f32 = 16.0;
const LIFT_MM: f32 = 5.0;
/// The rulers: far enough off the nearest plate's centre to clear its supports' pads.
const RULER_OFFSET_MM: f32 = 10.0;
const RULER_WIDTH_MM: f32 = 2.5;
const RULER_MM: f32 = 1.0;
const BUMP_MM: f32 = 1.0;
const BUMP_PITCH_MM: f32 = 1.6;

/// The coupon's plates and their supports as one mesh with its two rulers, built on `base`
/// with only the tip width changed and nothing merged or braced.
pub fn coupon(base: &SupportProfile, layer_mm: f32, centre: Vec3) -> Result<Mesh> {
    let mut whole = Mesh::default();
    let origin = Layout::new(centre).origin;
    for (row, tip_mm) in TIPS_MM.iter().enumerate() {
        let profile = profile_for(base, *tip_mm);
        for (column, holders) in HOLDERS.iter().enumerate() {
            let middle = origin + Vec3::new(column as f32 * PITCH_MM, row as f32 * PITCH_MM, 0.0);
            append(
                &mut whole,
                &held_plate(&profile, middle, *holders, layer_mm)?,
            );
        }
    }
    append(&mut whole, &rulers(origin));
    Ok(whole)
}

/// Two rails cured flat on the plate, where nothing can tear them off: beside each row as
/// many bumps as its place, 1 for the finest tip, and beside each column as many as the
/// tips under its plates. They mark no plate, so every plate is tested alike.
fn rulers(origin: Vec3) -> Mesh {
    let mut mesh = Mesh::default();
    let rows = TIPS_MM.len() as f32 * PITCH_MM;
    let columns = HOLDERS.len() as f32 * PITCH_MM;
    let start = origin - Vec3::new(PITCH_MM / 2.0, PITCH_MM / 2.0, 0.0);
    let left = start.x + PITCH_MM / 2.0 - RULER_OFFSET_MM;
    let near = start.y + PITCH_MM / 2.0 - RULER_OFFSET_MM;
    let half = RULER_WIDTH_MM / 2.0;
    append(
        &mut mesh,
        &cuboid(
            Vec3::new(left - half, start.y, 0.0),
            Vec3::new(left + half, start.y + rows, RULER_MM),
        ),
    );
    append(
        &mut mesh,
        &cuboid(
            Vec3::new(start.x, near - half, 0.0),
            Vec3::new(start.x + columns, near + half, RULER_MM),
        ),
    );
    for row in 0..TIPS_MM.len() {
        let middle = origin.y + row as f32 * PITCH_MM;
        for at in bumps(row + 1, middle) {
            append(&mut mesh, &bump(Vec2::new(left, at)));
        }
    }
    for (column, holders) in HOLDERS.iter().enumerate() {
        let middle = origin.x + column as f32 * PITCH_MM;
        for at in bumps(*holders, middle) {
            append(&mut mesh, &bump(Vec2::new(at, near)));
        }
    }
    mesh
}

/// Where `count` bumps sit, centred on `middle` along the rail.
fn bumps(count: usize, middle: f32) -> impl Iterator<Item = f32> {
    let first = middle - BUMP_PITCH_MM * (count as f32 - 1.0) / 2.0;
    (0..count).map(move |at| first + at as f32 * BUMP_PITCH_MM)
}

fn bump(at: Vec2) -> Mesh {
    let half = BUMP_MM / 2.0;
    cuboid(
        Vec3::new(at.x - half, at.y - half, RULER_MM - 0.1),
        Vec3::new(at.x + half, at.y + half, RULER_MM + BUMP_MM),
    )
}

/// Where the plates stand: row by tip width along Y, column by tips along X.
pub struct Layout {
    origin: Vec3,
}

impl Layout {
    pub fn new(centre: Vec3) -> Self {
        let span = |count: usize| PITCH_MM * (count as f32 - 1.0) / 2.0;
        Self {
            origin: centre - Vec3::new(span(HOLDERS.len()), span(TIPS_MM.len()), 0.0),
        }
    }

    /// The row and column of the plate over `at`, plate millimetres.
    pub fn plate(&self, at: Vec2) -> Option<(usize, usize)> {
        let local = (at - self.origin.truncate()) / PITCH_MM;
        let (column, row) = (local.x.round(), local.y.round());
        let inside = (0.0..HOLDERS.len() as f32).contains(&column)
            && (0.0..TIPS_MM.len() as f32).contains(&row);
        inside.then_some((row as usize, column as usize))
    }
}

fn profile_for(base: &SupportProfile, tip_mm: f32) -> SupportProfile {
    let mut profile = base.clone();
    profile.tip.contact_diameter_mm = tip_mm;
    profile.middle.diameter_mm = profile.middle.diameter_mm.max(tip_mm * 2.0);
    profile.branching.enabled = false;
    profile.bracing.enabled = false;
    profile.raft.enabled = false;
    profile
}

/// One plate centred over `middle`, held by `holders` tips spread over its underside.
fn held_plate(
    profile: &SupportProfile,
    middle: Vec3,
    holders: usize,
    layer_mm: f32,
) -> Result<Mesh> {
    let half = PLATE_MM / 2.0;
    let low = Vec3::new(middle.x - half, middle.y - half, LIFT_MM);
    let plate = cuboid(low, low + Vec3::new(PLATE_MM, PLATE_MM, PLATE_THICK_MM));
    let points: Vec<SupportPoint> = spots(holders)
        .into_iter()
        .map(|(x, y)| SupportPoint::new(Vec3::new(middle.x + x, middle.y + y, LIFT_MM - layer_mm)))
        .collect();
    let bvh = Bvh::build(&plate);
    let placed = Placed::new(&plate, &bvh, Transform::default());
    let table = std::slice::from_ref(profile);
    let profiles = Profiles::new(table).context("a table of one profile is never empty")?;
    let trees = grow(&columns(&points, &placed, profiles), &placed, profiles);
    let mut mesh = plate.clone();
    append(&mut mesh, &mesh_trees(&trees, &placed, profiles));
    Ok(mesh)
}

/// Where `holders` tips go under a plate, millimetres off its middle.
fn spots(holders: usize) -> Vec<(f32, f32)> {
    match holders {
        1 => vec![(0.0, 0.0)],
        2 => vec![(-2.0, -2.0), (2.0, 2.0)],
        4 => vec![(-2.0, -2.0), (2.0, -2.0), (-2.0, 2.0), (2.0, 2.0)],
        _ => (0..9)
            .map(|at| (2.5 * (at % 3) as f32 - 2.5, 2.5 * (at / 3) as f32 - 2.5))
            .collect(),
    }
}

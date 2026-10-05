//! A bench for hollowing: what each stage costs in time and in live bytes.
//!
//! Peak memory is counted by the allocator rather than read off the process, so it is the
//! bytes this run actually holds — no allocator slack, no page-cache noise, and the same
//! number on every machine. See `docs/design/hollowing.md`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use anyhow::{Context, Result};
use core_geometry::{Bvh, Mesh, Scalar, orient_outward, weld};
use core_mesh_io::{MeshLoader, StlLoader};
use core_volume::{
    FieldSettings, HollowMode, HollowSettings, InfillPattern, InfillSettings, SignMode, build,
    extract, hollow,
};

/// Live bytes now and the most there have ever been.
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let grown = unsafe { System.realloc(pointer, layout, new_size) };
        if !grown.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            record(new_size);
        }
        grown
    }
}

fn record(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Live bytes now.
fn live_mb() -> f64 {
    LIVE.load(Ordering::Relaxed) as f64 / 1e6
}

/// The most live bytes there have been since the last reset, and resets the mark.
fn take_peak_mb() -> f64 {
    let peak = PEAK.swap(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
    peak as f64 / 1e6
}

/// One row of the matrix: what was asked for.
struct Case {
    label: &'static str,
    wall_mm: Scalar,
    precision: Scalar,
    infill: Option<(InfillPattern, Scalar, Scalar)>,
    mode: HollowMode,
    /// Lattice to use whatever precision asks for, so two walls can be compared on one.
    lattice_mm: Option<Scalar>,
    sign: SignMode,
}

fn matrix() -> Vec<Case> {
    use InfillPattern::{Grid, Hive};
    vec![
        Case {
            label: "2 mm wall, coarse",
            wall_mm: 2.0,
            precision: 0.0,
            infill: None,
            mode: HollowMode::Internal,
            lattice_mm: None,
            sign: SignMode::Auto,
        },
        Case {
            label: "2 mm wall, default",
            wall_mm: 2.0,
            precision: 0.5,
            infill: None,
            mode: HollowMode::Internal,
            lattice_mm: None,
            sign: SignMode::Auto,
        },
        Case {
            label: "2 mm wall, finest",
            wall_mm: 2.0,
            precision: 1.0,
            infill: None,
            mode: HollowMode::Internal,
            lattice_mm: None,
            sign: SignMode::Auto,
        },
        Case {
            label: "0.5 mm wall, finest",
            wall_mm: 0.5,
            precision: 1.0,
            infill: None,
            mode: HollowMode::Internal,
            lattice_mm: None,
            sign: SignMode::Auto,
        },
        Case {
            label: "2 mm wall, 5 mm grid at 15%",
            wall_mm: 2.0,
            precision: 0.5,
            infill: Some((Grid, 5.0, 0.15)),
            mode: HollowMode::Internal,
            lattice_mm: None,
            sign: SignMode::Auto,
        },
        Case {
            label: "0.5 mm wall, 1 mm hive at 50%, finest",
            wall_mm: 0.5,
            precision: 1.0,
            infill: Some((Hive, 1.0, 0.5)),
            mode: HollowMode::Internal,
            lattice_mm: None,
            sign: SignMode::Auto,
        },
        // The same lattice under two walls: what the wall alone costs.
        Case {
            label: "0.5 mm wall at 0.1 mm",
            wall_mm: 0.5,
            precision: 1.0,
            infill: None,
            mode: HollowMode::Internal,
            lattice_mm: Some(0.1),
            sign: SignMode::Auto,
        },
        Case {
            label: "2 mm wall at 0.1 mm",
            wall_mm: 2.0,
            precision: 1.0,
            infill: None,
            mode: HollowMode::Internal,
            lattice_mm: Some(0.1),
            sign: SignMode::Auto,
        },
        Case {
            label: "6 mm wall at 0.1 mm",
            wall_mm: 6.0,
            precision: 1.0,
            infill: None,
            mode: HollowMode::Internal,
            lattice_mm: Some(0.1),
            sign: SignMode::Auto,
        },
    ]
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(
        args.next()
            .context("usage: hollow-lab <model.stl> [case index]")?,
    );
    let rest: Vec<String> = args.collect();
    let only: Option<usize> = rest.first().and_then(|index| index.parse().ok());
    let asked = asked_case(&rest);

    let mesh = load(&path)?;
    let bounds = mesh.aabb().context("the model has no vertices")?;
    let span = bounds.maxs - bounds.mins;
    println!(
        "{} — {} faces, {:.1} x {:.1} x {:.1} mm, {:.1} MB resident",
        path.display(),
        mesh.faces.len(),
        span.x,
        span.y,
        span.z,
        live_mb()
    );

    let started = Instant::now();
    let bvh = Bvh::build(&mesh);
    println!(
        "bvh {:.2} s, {:.1} MB live\n",
        started.elapsed().as_secs_f64(),
        live_mb()
    );

    println!("| case | lattice | stage | time | peak MB | size |");
    println!("|---|---|---|---|---|---|");
    match asked {
        Some(case) => run(&mesh, &bvh, &case)?,
        None => {
            for (index, case) in matrix().iter().enumerate() {
                if only.is_some_and(|wanted| wanted != index) {
                    continue;
                }
                run(&mesh, &bvh, case)?;
            }
        }
    }
    Ok(())
}

/// One case named on the command line: `--wall 0.3 --precision 0.5 [--lattice 0.1]`.
fn asked_case(args: &[String]) -> Option<Case> {
    let value = |name: &str| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|at| args.get(at + 1))
            .and_then(|raw| raw.parse::<Scalar>().ok())
    };
    Some(Case {
        label: "asked",
        wall_mm: value("--wall")?,
        precision: value("--precision").unwrap_or(0.5),
        infill: None,
        mode: HollowMode::Internal,
        lattice_mm: value("--lattice"),
        sign: if args.iter().any(|arg| arg == "--pseudonormal") {
            SignMode::Pseudonormal
        } else {
            SignMode::Auto
        },
    })
}

/// One case, with the field and the extraction timed on their own before the whole run.
fn run(mesh: &Mesh, bvh: &Bvh, case: &Case) -> Result<()> {
    let settings = HollowSettings {
        thickness_mm: case.wall_mm,
        mode: case.mode,
        precision: case.precision,
        infill: case
            .infill
            .map(|(pattern, size_mm, density)| InfillSettings {
                pattern,
                size_mm,
                density,
            }),
        sign: case.sign,
        ..HollowSettings::default()
    };
    let voxel_mm = case
        .lattice_mm
        .unwrap_or_else(|| settings.voxel_mm(mesh.surface_area()));

    take_peak_mb();
    let started = Instant::now();
    let field = build(
        mesh,
        bvh,
        &FieldSettings {
            voxel_mm,
            band_voxels: 2.0,
            iso_mm: -case.wall_mm,
            sign: settings.sign,
            ..FieldSettings::default()
        },
    );
    match field {
        Ok(field) => {
            row(
                case.label,
                voxel_mm,
                "field",
                started.elapsed().as_secs_f64(),
                &format!("{} tiles", field.tile_count()),
            );

            take_peak_mb();
            let started = Instant::now();
            let cavity = extract(&field);
            row(
                case.label,
                voxel_mm,
                "extract",
                started.elapsed().as_secs_f64(),
                &format!("{} faces", cavity.faces.len()),
            );
        }
        // The lattice precision asks for is priced before it is filled, and hollowing
        // answers a refusal by coarsening; the whole run below is what the user sees.
        Err(error) => println!(
            "| {} | {voxel_mm:.3} mm | field | — | — | {error} |",
            case.label
        ),
    }

    take_peak_mb();
    let started = Instant::now();
    match hollow(mesh, bvh, &settings) {
        Ok(hollowed) => row(
            case.label,
            hollowed.voxel_mm,
            if hollowed.coarsened {
                "whole (coarsened)"
            } else {
                "whole"
            },
            started.elapsed().as_secs_f64(),
            &format!("{} faces", hollowed.mesh.faces.len()),
        ),
        Err(error) => println!(
            "| {} | {voxel_mm:.3} mm | whole | — | — | refused: {error} |",
            case.label
        ),
    }
    Ok(())
}

fn row(label: &str, voxel_mm: Scalar, stage: &str, seconds: f64, size: &str) {
    println!(
        "| {label} | {voxel_mm:.3} mm | {stage} | {seconds:.2} s | {:.0} | {size} |",
        take_peak_mb()
    );
}

/// The model as the window would have it: welded, oriented, ready to query.
fn load(path: &std::path::Path) -> Result<Mesh> {
    let loaded = StlLoader
        .load(path)
        .with_context(|| format!("cannot load {}", path.display()))?;
    let mut mesh = weld(&loaded.mesh, core_geometry::DEFAULT_WELD_TOLERANCE).mesh;
    orient_outward(&mut mesh);
    Ok(mesh)
}

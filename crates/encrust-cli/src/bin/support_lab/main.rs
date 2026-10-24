//! A bench for automatic supports: a corpus supported as Generate supports it, cut and read.
//! See `docs/design/supports.md`; the corpus and rules of success live in local-docs/supports/.

mod corpus;
mod coupon;
mod cure;
mod export;
mod report;
mod shape;
mod stand;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use core_analysis::NECK_LIMIT_MPA;
use core_geometry::{Mesh, Quat, Vec2};
use printer_profiles::{Catalogue, MaterialProfile, PrinterProfile, SupportProfile};

use crate::stand::Asked;

use crate::corpus::{Entry, Expect, Pose};
use crate::report::{CureRow, Report, Row, SweepRow};

/// Shares of a profile's density a sweep tries: from a third of it to four times.
const SWEEP: [f32; 8] = [0.35, 0.5, 0.7, 1.0, 1.4, 2.0, 2.8, 4.0];

/// Benches automatic supports on models named outright or on a corpus.
#[derive(Parser)]
#[command(name = "support-lab")]
struct Args {
    /// Models to bench, stood as drawn.
    models: Vec<PathBuf>,
    /// A corpus file listing models, how each stands and what is expected of it.
    #[arg(long)]
    corpus: Option<PathBuf>,
    /// Bench only the models whose name holds this.
    #[arg(long)]
    only: Option<String>,
    #[arg(long, default_value = "elegoo-saturn-4-ultra")]
    printer: String,
    /// Support profiles to bench each model with; repeat for several. Light when neither
    /// this nor `--auto` is given.
    #[arg(long = "profile")]
    profiles: Vec<String>,
    /// Automatic profiles to bench each model with, each support sized by what it holds as
    /// Generate does with Automatic on; repeat for several.
    #[arg(long = "auto")]
    autos: Vec<String>,
    /// Hollow each model to a wall this thick, millimetres, and stand its supports
    /// against the shell from outside, as Generate does for a hollowed model.
    #[arg(long)]
    hollow_wall_mm: Option<f32>,
    /// Layer height, millimetres.
    #[arg(long, default_value_t = 0.05)]
    layer_mm: f32,
    /// Also support each model at shares of the profile's density, to find the least that
    /// holds. Slow: a full cure each.
    #[arg(long)]
    sweep: bool,
    /// Write each supported model and markers at what the bench names into this folder.
    #[arg(long)]
    export: Option<PathBuf>,
    /// Write the run as JSON, to keep as a baseline.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Compare with a run kept before; anything worse fails the run.
    #[arg(long)]
    baseline: Option<PathBuf>,
    /// Flat underside a tip holds per square millimetre of its section, as the coupons
    /// measured it (local-docs/supports/experiments.md).
    #[arg(long, default_value_t = 50.0)]
    flat_ratio: f32,
    /// The same for underside that leans.
    #[arg(long, default_value_t = 150.0)]
    slope_ratio: f32,
    /// The thinnest tip that holds at all, millimetres: a property of the resin and its
    /// exposure.
    #[arg(long, default_value_t = 0.3)]
    min_tip_mm: f32,
    /// The furthest a flat rim may stand from every tip, millimetres.
    #[arg(long, default_value_t = 1.5)]
    rim_gap_mm: f32,
    /// The stress a neck of green resin tears at, megapascals, once the coupon has measured
    /// it; no lower than the stack's own warning level, which it never names below.
    #[arg(long, default_value_t = NECK_LIMIT_MPA as f32)]
    neck_mpa: f32,
    /// Write a calibration coupon to this STL instead of benching, with what the bench
    /// expects of each specimen; laid out by `--coupon-spec`.
    #[arg(long, requires = "coupon_spec")]
    coupon: Option<PathBuf>,
    /// The TOML file a coupon is laid out by: tip shapes, and the blocks printed in them.
    #[arg(long)]
    coupon_spec: Option<PathBuf>,
}

/// What every model is printed with.
struct Setup {
    printer: PrinterProfile,
    material: MaterialProfile,
    limits: shape::Limits,
    neck_mpa: f32,
    wall_mm: Option<f32>,
}

fn main() -> Result<ExitCode> {
    let args = Args::parse();
    let catalogue = Catalogue::bundled().context("the bundled profiles do not load")?;
    let material = MaterialProfile {
        layer_height_mm: args.layer_mm,
        ..MaterialProfile::default()
    };
    let setup = Setup {
        printer: catalogue.printer(&args.printer)?.profile.clone(),
        material,
        limits: shape::Limits {
            flat_ratio: args.flat_ratio,
            slope_ratio: args.slope_ratio,
            min_tip_mm: args.min_tip_mm,
            rim_gap_mm: args.rim_gap_mm,
        },
        neck_mpa: args.neck_mpa.max(NECK_LIMIT_MPA as f32),
        wall_mm: args.hollow_wall_mm,
    };
    let asked = asked(&args, &catalogue)?;
    if let (Some(path), Some(spec)) = (&args.coupon, &args.coupon_spec) {
        let base = match asked.first() {
            Some((_, Asked::Profile(profile))) => profile.as_ref().clone(),
            _ => catalogue.support("light")?.profile.clone(),
        };
        write_coupon(path, &coupon::Spec::read(spec)?, &base, &setup)?;
        return Ok(ExitCode::SUCCESS);
    }

    let mut report = Report::new(&args.printer, args.layer_mm, setup.limits);
    for entry in entries(&args)? {
        for (id, asked) in &asked {
            let row = bench(&entry, id, asked, &setup, &args);
            report::print_row(&row);
            report.rows.push(row);
        }
    }
    report::print_table(&report);
    if let Some(path) = &args.json {
        report.write(path)?;
    }
    if let Some(path) = &args.baseline {
        let worse = report::regressions(&Report::read(path)?, &report);
        if !worse.is_empty() {
            println!("\nWorse than {}:", path.display());
            for line in &worse {
                println!("- {line}");
            }
            return Ok(ExitCode::FAILURE);
        }
        println!("\nNothing worse than {}", path.display());
    }
    Ok(ExitCode::SUCCESS)
}

/// What each model is benched with, and the name a row gives it: the profiles asked for,
/// then the automatic ones, each with the shapes it names as the catalogue has them.
fn asked(args: &Args, catalogue: &Catalogue) -> Result<Vec<(String, Asked)>> {
    let light = ["light".to_owned()];
    let profiles = match (args.profiles.is_empty(), args.autos.is_empty()) {
        (true, true) => &light[..],
        _ => &args.profiles[..],
    };
    let mut asked: Vec<(String, Asked)> = profiles
        .iter()
        .map(|id| {
            let profile = catalogue.support(id)?.profile.clone();
            Ok((id.clone(), Asked::Profile(Box::new(profile))))
        })
        .collect::<Result<_>>()?;
    for id in &args.autos {
        let auto = catalogue
            .autos()
            .find(|entry| entry.id == *id)
            .with_context(|| format!("no automatic profile {id}"))?
            .profile
            .clone();
        let mut shapes = Vec::new();
        for shape in auto.every_shape() {
            shapes.push(catalogue.support(shape)?.profile.clone());
        }
        let shapes = shapes.try_into().map_err(|_| {
            anyhow::anyhow!("automatic profile {id} names the wrong number of shapes")
        })?;
        asked.push((
            format!("auto:{id}"),
            Asked::Auto {
                auto: Box::new(auto),
                shapes: Box::new(shapes),
            },
        ));
    }
    Ok(asked)
}

fn entries(args: &Args) -> Result<Vec<Entry>> {
    let mut entries: Vec<Entry> = match &args.corpus {
        Some(path) => corpus::read(path)?,
        None => args
            .models
            .iter()
            .map(|path| Entry {
                name: name_of(path),
                path: path.clone(),
                note: String::new(),
                skip: None,
                pose: Pose::default(),
                expect: Expect::default(),
            })
            .collect(),
    };
    if let Some(only) = &args.only {
        entries.retain(|entry| entry.name.contains(only.as_str()));
    }
    Ok(entries)
}

pub fn name_of(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// One model with one profile; one that cannot be loaded or measured is reported skipped,
/// so a run over a corpus still finishes.
fn bench(entry: &Entry, id: &str, asked: &Asked, setup: &Setup, args: &Args) -> Row {
    let started = Instant::now();
    let mut row = Row {
        model: entry.name.clone(),
        profile: id.to_owned(),
        faces: 0,
        size_mm: [0.0; 3],
        skipped: entry.skip.clone(),
        shape: shape::Shape::default(),
        cure: CureRow::default(),
        sweep: Vec::new(),
        broken: Vec::new(),
        seconds: 0.0,
        stages_s: [0.0; 3],
    };
    if row.skipped.is_some() {
        return row;
    }
    if let Err(error) = measure(&mut row, entry, asked, setup, args) {
        row.skipped = Some(format!("{error:#}"));
    }
    row.seconds = started.elapsed().as_secs_f64();
    row
}

fn measure(row: &mut Row, entry: &Entry, asked: &Asked, setup: &Setup, args: &Args) -> Result<()> {
    let loaded = stand::load(&entry.path)?;
    let (posed, leant) = stand::posed(&loaded, entry.pose, &setup.printer.build_volume)?;
    row.faces = posed.faces.len();
    if let Some(bounds) = posed.aabb() {
        row.size_mm = (bounds.maxs - bounds.mins).to_array();
    }
    let (supported, shape, cured) = run(&posed, asked, leant, setup)?;
    row.cure = CureRow::of(&cured, setup.neck_mpa);
    row.stages_s = [supported.place_s, supported.grow_s, cured.seconds];
    row.broken = broken(&entry.expect, &shape);
    if let Some(dir) = &args.export {
        let stem = format!("{}-{}", row.model, row.profile);
        export::write(
            dir,
            &stem,
            &supported.model,
            &supported.supports,
            &shape.findings,
            &cured,
            setup.neck_mpa,
        )?;
    }
    row.shape = shape;
    if args.sweep {
        row.sweep = sweep(&posed, asked, leant, setup)?;
    }
    Ok(())
}

fn run(
    posed: &Mesh,
    asked: &Asked,
    leant: Quat,
    setup: &Setup,
) -> Result<(stand::Supported, shape::Shape, cure::Cured)> {
    let layer_mm = setup.material.layer_height_mm;
    let supported = stand::supported(posed.clone(), asked, setup.wall_mm, layer_mm)?;
    let shape = shape::read(&supported, &asked.placing(), &setup.limits, leant);
    let cured = cure::cure(
        &supported.model,
        &supported.supports,
        &setup.printer,
        &setup.material,
    )?;
    Ok((supported, shape, cured))
}

fn sweep(posed: &Mesh, asked: &Asked, leant: Quat, setup: &Setup) -> Result<Vec<SweepRow>> {
    SWEEP
        .iter()
        .map(|share| {
            let (_, shape, cured) = run(posed, &asked.thinned(*share), leant, setup)?;
            let cure = CureRow::of(&cured, setup.neck_mpa);
            let held = report::held(&shape, &cure);
            Ok(SweepRow {
                share: *share,
                contacts: shape.standing,
                supports_ml: shape.supports_ml,
                held,
                enough: report::enough(&shape, &cure),
                use_p90: shape.use_p90,
                levers: cure.levers,
                islands: cure.islands,
                bare_mm2: shape.bare_mm2,
            })
        })
        .collect()
}

fn broken(expect: &Expect, shape: &shape::Shape) -> Vec<String> {
    let mut broken = Vec::new();
    if let Some(most) = expect.max_contacts
        && shape.standing > most
    {
        broken.push(format!(
            "{} contacts, at most {most} expected",
            shape.standing
        ));
    }
    if let Some(most) = expect.max_supports_ml
        && shape.supports_ml > most
    {
        broken.push(format!(
            "{:.2} ml of supports, at most {most} expected",
            shape.supports_ml
        ));
    }
    broken
}

/// Writes the coupon and says which of its plates the cured stack expects to tear off.
fn write_coupon(
    path: &Path,
    spec: &coupon::Spec,
    base: &SupportProfile,
    setup: &Setup,
) -> Result<()> {
    let volume = &setup.printer.build_volume;
    let centre = Vec2::new(volume.x / 2.0, volume.y / 2.0);
    let built = coupon::build(spec, base, setup.material.layer_height_mm, centre)?;
    let mesh = coupon::coupon(spec, &built, centre)?;
    export::stl(path, &mesh)?;
    println!("wrote {}", path.display());
    let cured = cure::cure(&mesh, &Mesh::default(), &setup.printer, &setup.material)?;
    let specimens = coupon::layout(spec, centre);
    let verdicts: Vec<String> = specimens
        .iter()
        .zip(&built)
        .map(|(specimen, built)| verdict(specimen, built, &specimens, &cured, setup))
        .collect();
    println!("What the bench expects of each specimen:");
    for (index, block) in spec.blocks.iter().enumerate() {
        println!("\n{} (block {}):", block.name, index + 1);
        let labels: Vec<String> = block.holders.iter().map(|n| format!("{n} tips")).collect();
        println!(
            "| | {} |\n|---|{}",
            labels.join(" | "),
            "---|".repeat(labels.len())
        );
        for (row, shape) in spec.rows(block).iter().enumerate() {
            let cells: Vec<&str> = specimens
                .iter()
                .zip(&verdicts)
                .filter(|(s, _)| s.block == index && s.row == row)
                .map(|(_, verdict)| verdict.as_str())
                .collect();
            println!("| {} | {} |", shape.name, cells.join(" | "));
        }
    }
    Ok(())
}

/// One specimen as the bench reads it: an island in it is the coupon's own fault, then a
/// tip too thin, a tip past what it holds, a corner too far from any tip.
fn verdict(
    specimen: &coupon::Specimen,
    built: &coupon::Built,
    specimens: &[coupon::Specimen],
    cured: &cure::Cured,
    setup: &Setup,
) -> String {
    let islands = cured
        .risks
        .iter()
        .filter(|placed| matches!(placed.risk.kind, core_analysis::RiskKind::Island { .. }))
        .filter(|placed| {
            coupon::specimen_at(specimens, placed.at)
                .is_some_and(|found| std::ptr::eq(found, specimen))
        })
        .count();
    let shape = shape::read(
        &built.supported,
        &built.profile,
        &setup.limits,
        Quat::IDENTITY,
    );
    if islands > 0 {
        format!("**{islands} islands**")
    } else if shape.weak_tips > 0 {
        "**tip too thin**".to_owned()
    } else if shape.overloaded > 0 {
        format!("**tears, use {:.1}**", shape.use_max)
    } else if shape.rim_gap_max_mm > setup.limits.rim_gap_mm {
        format!(
            "holds, use {:.1}, corner {:.1} mm off",
            shape.use_max, shape.rim_gap_max_mm
        )
    } else {
        format!("holds, use {:.1}", shape.use_max)
    }
}

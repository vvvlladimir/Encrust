//! A bench for automatic supports: each model of a corpus supported as Generate supports
//! it, cut and cured as it prints, and read for what falls, what marks the part and what is
//! more than it needs. See `docs/design/supports.md`; the corpus and the rules of success
//! live outside the repository, in local-docs/supports/.

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
use core_analysis::{NECK_LIMIT_MPA, PULL_N_PER_MM2};
use core_geometry::{Mesh, Quat, Vec3};
use printer_profiles::{Catalogue, MaterialProfile, PrinterProfile, SupportProfile};

use crate::corpus::{Entry, Expect, Pose};
use crate::report::{CureRow, Report, Row, SweepRow};

/// Shares of a profile's density a sweep tries: from a third of it to half again.
const SWEEP: [f32; 6] = [0.35, 0.5, 0.7, 1.0, 1.4, 2.0];

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
    /// Support profiles to bench each model with; repeat for several.
    #[arg(long = "profile", default_values_t = ["light".to_owned()])]
    profiles: Vec<String>,
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
    /// How many times over a tip is to hold the pull it carries, against green resin's
    /// strength: sets how much underside a tip may carry.
    #[arg(long, default_value_t = 4.0)]
    safety: f32,
    /// The stress a neck of green resin tears at, megapascals, once the coupon has measured
    /// it; no lower than the stack's own warning level, which it never names below.
    #[arg(long, default_value_t = NECK_LIMIT_MPA as f32)]
    neck_mpa: f32,
    /// Write the tip calibration coupon to this STL instead of benching, with what the
    /// bench expects of each of its plates.
    #[arg(long)]
    coupon: Option<PathBuf>,
}

/// What every model is printed with.
struct Setup {
    printer: PrinterProfile,
    material: MaterialProfile,
    /// The most underside a tip may carry per square millimetre of its section.
    carry_limit: f32,
    neck_mpa: f32,
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
        carry_limit: args.neck_mpa / PULL_N_PER_MM2 as f32 / args.safety,
        neck_mpa: args.neck_mpa.max(NECK_LIMIT_MPA as f32),
    };
    let profiles: Vec<SupportProfile> = args
        .profiles
        .iter()
        .map(|id| Ok(catalogue.support(id)?.profile.clone()))
        .collect::<Result<_>>()?;
    if let Some(path) = &args.coupon {
        write_coupon(path, &profiles[0], &setup)?;
        return Ok(ExitCode::SUCCESS);
    }

    let mut report = Report::new(&args.printer, args.layer_mm, setup.carry_limit);
    for entry in entries(&args)? {
        for (id, profile) in args.profiles.iter().zip(&profiles) {
            let row = bench(&entry, id, profile, &setup, &args);
            report::print_row(&row, setup.carry_limit);
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
fn bench(entry: &Entry, id: &str, profile: &SupportProfile, setup: &Setup, args: &Args) -> Row {
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
    if let Err(error) = measure(&mut row, entry, profile, setup, args) {
        row.skipped = Some(format!("{error:#}"));
    }
    row.seconds = started.elapsed().as_secs_f64();
    row
}

fn measure(
    row: &mut Row,
    entry: &Entry,
    profile: &SupportProfile,
    setup: &Setup,
    args: &Args,
) -> Result<()> {
    let loaded = stand::load(&entry.path)?;
    let (posed, leant) = stand::posed(&loaded, entry.pose, &setup.printer.build_volume)?;
    row.faces = posed.faces.len();
    if let Some(bounds) = posed.aabb() {
        row.size_mm = (bounds.maxs - bounds.mins).to_array();
    }
    let (supported, shape, cured) = run(&posed, profile, leant, setup)?;
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
        row.sweep = sweep(&posed, profile, leant, setup)?;
    }
    Ok(())
}

fn run(
    posed: &Mesh,
    profile: &SupportProfile,
    leant: Quat,
    setup: &Setup,
) -> Result<(stand::Supported, shape::Shape, cure::Cured)> {
    let supported = stand::supported(posed.clone(), profile, setup.material.layer_height_mm)?;
    let shape = shape::read(&supported, profile, leant);
    let cured = cure::cure(
        &supported.model,
        &supported.supports,
        &setup.printer,
        &setup.material,
    )?;
    Ok((supported, shape, cured))
}

fn sweep(
    posed: &Mesh,
    profile: &SupportProfile,
    leant: Quat,
    setup: &Setup,
) -> Result<Vec<SweepRow>> {
    SWEEP
        .iter()
        .map(|share| {
            let mut thinned = profile.clone();
            thinned.density *= share;
            let (_, shape, cured) = run(posed, &thinned, leant, setup)?;
            let cure = CureRow::of(&cured, setup.neck_mpa);
            let held = report::held(&shape, &cure);
            Ok(SweepRow {
                share: *share,
                contacts: shape.standing,
                supports_ml: shape.supports_ml,
                held,
                enough: held && shape.carry_ratio() <= setup.carry_limit,
                carry_ratio: shape.carry_ratio(),
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
fn write_coupon(path: &Path, base: &SupportProfile, setup: &Setup) -> Result<()> {
    let volume = &setup.printer.build_volume;
    let centre = Vec3::new(volume.x / 2.0, volume.y / 2.0, 0.0);
    let mesh = coupon::coupon(base, setup.material.layer_height_mm, centre)?;
    export::stl(path, &mesh)?;
    println!("wrote {}", path.display());
    let cured = cure::cure(&mesh, &Mesh::default(), &setup.printer, &setup.material)?;
    let at = coupon::Layout::new(centre);
    let mut stress = [[0.0f32; coupon::HOLDERS.len()]; coupon::TIPS_MM.len()];
    for placed in &cured.risks {
        if let core_analysis::RiskKind::Lever { stress_mpa, .. } = placed.risk.kind
            && stress_mpa >= setup.neck_mpa
            && let Some((row, column)) = at.plate(placed.at)
        {
            stress[row][column] = stress[row][column].max(stress_mpa);
        }
    }
    println!(
        "hardest pull {:.2} N. Neck stress the stack expects, MPa, torn past {}:",
        cured.hardest_pull_n, setup.neck_mpa
    );
    println!(
        "| tip mm | {} |",
        coupon::HOLDERS.map(|n| format!("{n} tips")).join(" | ")
    );
    println!("|---|{}", "---|".repeat(coupon::HOLDERS.len()));
    for (row, tip) in coupon::TIPS_MM.iter().enumerate() {
        let cells: Vec<String> = stress[row]
            .iter()
            .map(|mpa| {
                if *mpa > 0.0 {
                    format!("**{mpa:.0}**")
                } else {
                    "holds".to_owned()
                }
            })
            .collect();
        println!("| {tip} | {} |", cells.join(" | "));
    }
    Ok(())
}

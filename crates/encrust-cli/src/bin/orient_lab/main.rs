//! A bench for orientation: each goal, and the poses a user would try by hand, judged by the
//! supports and the stack they actually produce rather than by the score that chose them.
//! See `docs/design/orientation.md`.

mod bodies;

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use core_engine::{Printout, Trial, orient_proven, print_out};
use core_geometry::{Mesh, Quat, Vec3, orient_outward, weld};
use core_plate::{OrientGoal, OrientSettings, Printed, flat_faces, rotation_for};
use encrust_cli::pipeline;
use printer_profiles::{Catalogue, MaterialProfile, PrinterProfile, SupportProfile};

/// A goal passes when its own reading is within this share of the best pose by hand.
const WITHIN: f64 = 0.10;

/// How far over the fewest failures by hand a pose may be and still print as well: a
/// quarter and two, since an island more or less is the support generator's noise.
const FAILURE_BAND: f64 = 1.25;
const FAILURE_SLACK: f64 = 2.0;

/// What every pose is printed with.
struct Setup {
    printer: PrinterProfile,
    material: MaterialProfile,
    support: SupportProfile,
}

impl Setup {
    fn trial(&self) -> Trial<'_> {
        Trial {
            printer: &self.printer,
            material: &self.material,
            support: &self.support,
            raster_window: rayon::current_num_threads(),
        }
    }
}

/// One way of standing a model, where it came from, and what it prints.
struct Pose {
    name: String,
    rotation: Quat,
    goal: Option<OrientGoal>,
    on_plate: bool,
    printout: Printout,
    seconds: f64,
}

fn main() -> Result<()> {
    let (paths, printer_id, support_id) = parse_args()?;
    let catalogue = Catalogue::bundled().context("the bundled profiles do not load")?;
    let setup = Setup {
        printer: catalogue.printer(&printer_id)?.profile.clone(),
        material: MaterialProfile::default(),
        support: catalogue.support(&support_id)?.profile.clone(),
    };
    println!(
        "printer {printer_id}, supports {support_id}, layer {} mm\n",
        setup.material.layer_height_mm
    );

    let models = if paths.is_empty() {
        bodies::all()
            .into_iter()
            .map(|(name, mesh)| (name.to_owned(), mesh))
            .collect()
    } else {
        paths
            .iter()
            .map(|path| load(path))
            .collect::<Result<Vec<_>>>()?
    };
    let mut passed = 0;
    let mut judged = 0;
    for (name, mesh) in &models {
        let (won, of) = bench(name, mesh, &setup)?;
        passed += won;
        judged += of;
    }
    println!("{passed} of {judged} goals pass");
    Ok(())
}

/// Model paths, then the printer and support profile ids, defaulting to the machine the
/// test prints run on.
fn parse_args() -> Result<(Vec<PathBuf>, String, String)> {
    let mut paths = Vec::new();
    let mut printer = "elegoo-saturn-4-ultra".to_owned();
    let mut support = "medium".to_owned();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--printer" => printer = args.next().context("--printer needs a profile id")?,
            "--support" => support = args.next().context("--support needs a profile id")?,
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    Ok((paths, printer, support))
}

fn load(path: &Path) -> Result<(String, Mesh)> {
    let loaded = pipeline::load(path)?;
    let mut mesh = weld(&loaded.mesh, core_geometry::DEFAULT_WELD_TOLERANCE).mesh;
    orient_outward(&mut mesh);
    let name = path.file_stem().unwrap_or_default().to_string_lossy();
    Ok((name.into_owned(), mesh))
}

/// Prints every pose of one model as a table, with how each goal does against the best
/// pose by hand, and answers how many goals passed of how many.
fn bench(name: &str, mesh: &Mesh, setup: &Setup) -> Result<(usize, usize)> {
    println!("## {name} — {} faces\n", mesh.faces.len());
    println!(
        "| pose | turn ° | on plate | contacts | hanging | on top | layers | pull N | growth mm² | islands | cups | peels | resin ml | seconds |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");

    let poses = poses(mesh, setup).with_context(|| format!("cannot measure {name}"))?;
    for pose in &poses {
        print_row(pose);
    }
    println!();
    let mut passed = 0;
    let mut judged = 0;
    for pose in &poses {
        if let Some(goal) = pose.goal {
            let (passes, call) = verdict(goal, pose, &poses);
            println!("- {}: {call}", goal.label());
            passed += usize::from(passes);
            judged += 1;
        }
    }
    println!();
    Ok((passed, judged))
}

/// The poses a user would try by hand, then what each goal chooses for a model standing
/// on the setup's supports, the way the window asks it.
fn poses(mesh: &Mesh, setup: &Setup) -> Result<Vec<Pose>> {
    let printed = Printed::solid(mesh);
    let trial = setup.trial();
    let largest = flat_faces(mesh)
        .first()
        .map_or(Vec3::NEG_Z, |flat| flat.normal);
    let corner = Vec3::new(1.0, 1.0, 0.0).normalize();
    let by_hand = [
        ("as is", Quat::IDENTITY, false),
        ("as is, on the plate", Quat::IDENTITY, true),
        ("largest face down", rotation_for(largest), false),
        (
            "tilted 35° on a corner",
            Quat::from_axis_angle(corner, 35f32.to_radians()),
            false,
        ),
    ];
    let mut poses = Vec::new();
    for (name, rotation, on_plate) in by_hand {
        let started = Instant::now();
        poses.push(Pose {
            name: name.to_owned(),
            rotation,
            goal: None,
            on_plate,
            printout: print_out(&printed, rotation, on_plate, &trial)?,
            seconds: started.elapsed().as_secs_f64(),
        });
    }
    for goal in OrientGoal::ALL {
        let settings = OrientSettings {
            goal,
            ..pipeline::orient_settings(Some(&setup.support))
        };
        let started = Instant::now();
        let found = orient_proven(&printed, &settings, &trial)
            .context("the search found no orientation")?
            .oriented;
        let seconds = started.elapsed().as_secs_f64();
        // The proof ranks on a coarse stack; the pose is judged on the one it prints.
        poses.push(Pose {
            name: format!("goal: {}", goal.label()),
            rotation: found.rotation,
            goal: Some(goal),
            on_plate: found.on_plate,
            printout: print_out(&printed, found.rotation, found.on_plate, &trial)?,
            seconds,
        });
    }
    Ok(poses)
}

fn print_row(pose: &Pose) {
    let turn_deg = pose.rotation.to_axis_angle().1.to_degrees();
    let o = &pose.printout;
    println!(
        "| {} | {turn_deg:.0} | {} | {} | {} | {} | {} | {:.1} | {:.0} | {} | {} | {} | {:.2} | {:.1} |",
        pose.name,
        if pose.on_plate { "yes" } else { "—" },
        o.contacts,
        o.hanging,
        o.top_contacts,
        o.layers,
        o.pull_n,
        o.growth_mm2,
        o.islands,
        o.cups,
        o.peels,
        o.resin_ml,
        pose.seconds,
    );
}

/// How a goal's pose does against the poses by hand. It passes when it fails no more
/// than the soundest of them, give or take the band, and either reads within `WITHIN` of
/// the best of those on the goal's own reading or is rid of a cup or a tearing pull every
/// one of them has: what loses the print comes before what the goal asks for.
fn verdict(goal: OrientGoal, chosen: &Pose, poses: &[Pose]) -> (bool, String) {
    let by_hand = || poses.iter().filter(|pose| pose.goal.is_none());
    let fewest = by_hand()
        .map(|pose| pose.printout.failures())
        .min()
        .unwrap_or(0);
    let limit = fewest as f64 * FAILURE_BAND + FAILURE_SLACK;
    let sound = || by_hand().filter(|pose| pose.printout.failures() as f64 <= limit);
    let (best_name, best) = sound()
        .map(|pose| (pose.name.as_str(), pose.printout.reading(goal)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or(("nothing", f64::INFINITY));
    let least_fatal = sound().map(|pose| pose.printout.fatal()).min().unwrap_or(0);

    let own = chosen.printout.reading(goal);
    let failures = chosen.printout.failures();
    let reads = own <= best * (1.0 + WITHIN) + f64::EPSILON;
    let sounder = chosen.printout.fatal() < least_fatal;
    let prints = failures as f64 <= limit;
    let call = match (prints, reads, sounder) {
        (true, true, _) => "passes",
        (true, false, true) => "passes, sounder",
        (true, false, false) => "LOSES",
        (false, true, _) => "FAILS MORE",
        (false, false, _) => "LOSES and FAILS MORE",
    };
    (
        prints && (reads || sounder),
        format!(
            "{call}: {own:.1} against {best:.1} for {best_name}, {failures} failures against {fewest}"
        ),
    )
}

//! One run of the bench as rows a person reads and a later run compares with.

use std::path::Path;

use anyhow::{Context, Result};
use core_analysis::RiskKind;
use serde::{Deserialize, Serialize};

use crate::cure::Cured;
use crate::shape::{Kind, Limits, Shape};

const SCHEMA: u32 = 2;
/// Growth in contacts or resin a run may show over its baseline before it is a regression:
/// placement moves a little with any change, and a few percent is not a decision.
const GROWTH: f32 = 1.15;
/// The smallest island that fails a print, square millimetres: a disc 0.4 mm across, the
/// least placement holds (`MIN_ISLAND_MM` in docs/design/supports.md).
pub const MIN_ISLAND_MM2: f32 = std::f32::consts::PI * 0.2 * 0.2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub schema: u32,
    pub printer: String,
    pub layer_mm: f32,
    /// What a tip was taken to hold.
    pub limits: Limits,
    pub rows: Vec<Row>,
}

/// One model supported to one profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub model: String,
    pub profile: String,
    pub faces: usize,
    /// How big it printed, millimetres.
    pub size_mm: [f32; 3],
    pub skipped: Option<String>,
    pub shape: Shape,
    pub cure: CureRow,
    /// Densities tried below and above the profile's, as a share of it.
    pub sweep: Vec<SweepRow>,
    /// Limits the corpus sets for this model that the run broke.
    pub broken: Vec<String>,
    pub seconds: f64,
    /// Placement, growing and curing, seconds.
    pub stages_s: [f64; 3],
}

/// What the cured stack named.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CureRow {
    pub layers: usize,
    pub resin_ml: f32,
    pub hardest_pull_n: f32,
    /// Islands at least `MIN_ISLAND_MM2` across: a part left in the vat.
    pub islands: usize,
    /// Islands smaller than that: a speck on the film, or the first layer of a point
    /// nothing caught, which the picture tells apart.
    pub specks: usize,
    pub floating: usize,
    /// Necks the pull is taken to break: a tip torn off or a detail snapped.
    pub levers: usize,
    pub worst_lever_mpa: f32,
    pub peels: usize,
    pub cups: usize,
    pub thins: usize,
    /// Where the print fails, worst first.
    pub failures: Vec<Failure>,
}

/// A place the cured stack says the print fails, plate millimetres.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Failure {
    /// `island`, `speck`, `floating` or `torn`.
    pub kind: String,
    pub at: [f32; 3],
    /// Area for an island, stress in megapascals for a torn neck.
    pub value: f32,
}

impl CureRow {
    /// What `cured` names, a neck counted torn from `neck_mpa`.
    pub fn of(cured: &Cured, neck_mpa: f32) -> Self {
        let torn = move |kind: &RiskKind| matches!(kind, RiskKind::Lever { stress_mpa, .. } if *stress_mpa >= neck_mpa);
        Self {
            layers: cured.layers,
            resin_ml: cured.resin_ml,
            hardest_pull_n: cured.hardest_pull_n,
            islands: cured.count(
                |kind| matches!(kind, RiskKind::Island { area_mm2 } if *area_mm2 >= MIN_ISLAND_MM2),
            ),
            specks: cured.count(
                |kind| matches!(kind, RiskKind::Island { area_mm2 } if *area_mm2 < MIN_ISLAND_MM2),
            ),
            floating: cured.count(|kind| matches!(kind, RiskKind::Floating { .. })),
            levers: cured
                .risks
                .iter()
                .filter(|placed| torn(&placed.risk.kind))
                .count(),
            worst_lever_mpa: cured.worst_lever_mpa(),
            peels: cured.count(|kind| matches!(kind, RiskKind::Peel { .. })),
            cups: cured.count(|kind| matches!(kind, RiskKind::SuctionCup { .. })),
            thins: cured.count(|kind| matches!(kind, RiskKind::Thin { .. })),
            failures: failures(cured, neck_mpa),
        }
    }
}

fn failures(cured: &Cured, neck_mpa: f32) -> Vec<Failure> {
    let mut failures: Vec<Failure> = cured
        .risks
        .iter()
        .filter_map(|placed| {
            let (kind, value) = match placed.risk.kind {
                RiskKind::Island { area_mm2 } if area_mm2 >= MIN_ISLAND_MM2 => ("island", area_mm2),
                RiskKind::Island { area_mm2 } => ("speck", area_mm2),
                RiskKind::Floating { area_mm2 } => ("floating", area_mm2),
                RiskKind::Lever { stress_mpa, .. } if stress_mpa >= neck_mpa => {
                    ("torn", stress_mpa)
                }
                _ => return None,
            };
            Some(Failure {
                kind: kind.to_owned(),
                at: [placed.at.x, placed.at.y, placed.z_mm],
                value,
            })
        })
        .collect();
    failures.sort_by(|a, b| b.value.total_cmp(&a.value));
    failures
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepRow {
    pub share: f32,
    pub contacts: usize,
    pub supports_ml: f32,
    pub held: bool,
    /// Held, and no tip carries more than it holds: enough supports.
    pub enough: bool,
    pub use_p90: f32,
    pub levers: usize,
    pub islands: usize,
    pub bare_mm2: f32,
}

impl Row {
    /// Nothing falls: every contact stands, nothing cures over nothing, no tip is thinner
    /// than holds at all.
    pub fn held(&self) -> bool {
        held(&self.shape, &self.cure)
    }

    /// Held, and no tip carries more than it holds.
    pub fn enough(&self) -> bool {
        enough(&self.shape, &self.cure)
    }

    /// The lowest share of the profile's density that is still enough, once a sweep ran.
    pub fn knee(&self) -> Option<f32> {
        self.sweep
            .iter()
            .filter(|row| row.enough)
            .map(|row| row.share)
            .reduce(f32::min)
    }

    /// Contacts per square centimetre of underside that needs them.
    pub fn per_cm2(&self) -> f32 {
        let cm2 = self.shape.overhang_mm2 / 100.0;
        if cm2 > 0.0 {
            self.shape.standing as f32 / cm2
        } else {
            0.0
        }
    }
}

/// Torn necks are left out until the coupon calibrates them: the stack reads every tip
/// under a piece as one disc, which no spread of tips bends like (README, «Калибровка»).
pub fn held(shape: &Shape, cure: &CureRow) -> bool {
    shape.contacts == shape.standing
        && cure.islands == 0
        && cure.floating == 0
        && shape.weak_tips == 0
}

pub fn enough(shape: &Shape, cure: &CureRow) -> bool {
    held(shape, cure) && shape.overloaded == 0
}

impl Report {
    pub fn new(printer: &str, layer_mm: f32, limits: Limits) -> Self {
        Self {
            schema: SCHEMA,
            printer: printer.to_owned(),
            layer_mm,
            limits,
            rows: Vec::new(),
        }
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))
    }

    pub fn read(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let report: Self = serde_json::from_str(&text)
            .with_context(|| format!("{} is not a support-lab run", path.display()))?;
        anyhow::ensure!(
            report.schema == SCHEMA,
            "{} is schema {}, not {SCHEMA}",
            path.display(),
            report.schema
        );
        Ok(report)
    }
}

/// One model's block: the readings, then what to look at, worst first.
pub fn print_row(row: &Row) {
    let shape = &row.shape;
    let cure = &row.cure;
    println!("## {} · {}", row.model, row.profile);
    if let Some(why) = &row.skipped {
        println!("skipped: {why}\n");
        return;
    }
    let [x, y, z] = row.size_mm;
    println!(
        "{x:.0}×{y:.0}×{z:.0} mm, {} faces, {:.2} ml · {:.1} s (place {:.1}, grow {:.1}, cure {:.1})",
        row.faces, shape.model_ml, row.seconds, row.stages_s[0], row.stages_s[1], row.stages_s[2]
    );
    println!(
        "- holds: {}, enough: {} · contacts {}/{} standing, tips too thin {}, islands {}, specks {}, floating {}, torn necks {} (worst {:.1} MPa)",
        if row.held() { "yes" } else { "NO" },
        if row.enough() { "yes" } else { "NO" },
        shape.standing,
        shape.contacts,
        shape.weak_tips,
        cure.islands,
        cure.specks,
        cure.floating,
        cure.levers,
        cure.worst_lever_mpa,
    );
    println!(
        "- cost: {:.2} ml supports ({:.0}% of the model), {} trees, {:.1} contacts/cm² of underside",
        shape.supports_ml,
        100.0 * shape.supports_ml / shape.model_ml.max(1e-3),
        shape.trees,
        row.per_cm2(),
    );
    println!(
        "- load: p90 {:.1} mm² a tip against {:.1} by design; use of what a tip holds median {:.2}, p90 {:.2}, max {:.2}; overloaded {}; underside {:.0} mm², {:.0} of it flat, bare {:.1}; flat rim far from tips {:.1} mm (worst {:.1} mm)",
        shape.load_p90_mm2,
        shape.design_mm2,
        shape.use_p50,
        shape.use_p90,
        shape.use_max,
        shape.overloaded,
        shape.overhang_mm2,
        shape.flat_mm2,
        shape.bare_mm2,
        shape.rim_bare_mm,
        shape.rim_gap_max_mm,
    );
    println!(
        "- marks: {:.1} mm² of scar, {} on the visible side, {} on thin details, {} feet on the model, {} crowded, {} slender",
        shape.scar_mm2,
        shape.visible,
        shape.thin_tips,
        shape.on_model,
        shape.crowded,
        shape.slender,
    );
    println!(
        "- stack: {} layers, hardest pull {:.1} N, peels {}, cups {}, thin runs {}",
        cure.layers, cure.hardest_pull_n, cure.peels, cure.cups, cure.thins,
    );
    print_sweep(row);
    for broken in &row.broken {
        println!("- EXPECTED: {broken}");
    }
    print_failures(cure);
    print_findings(shape);
    println!();
}

fn print_sweep(row: &Row) {
    if row.sweep.is_empty() {
        return;
    }
    let cells: Vec<String> = row
        .sweep
        .iter()
        .map(|step| {
            format!(
                "×{:.2}: {} tips {:.2} ml use {:.2} {}",
                step.share,
                step.contacts,
                step.supports_ml,
                step.use_p90,
                match (step.held, step.enough) {
                    (true, true) => "enough",
                    (true, false) => "overloaded",
                    _ => "falls",
                }
            )
        })
        .collect();
    println!("- density sweep: {}", cells.join(" · "));
    match row.knee() {
        Some(knee) => println!(
            "- balance: enough from ×{knee:.2} of the profile's density; the profile keeps ×{:.1} of that in hand",
            1.0 / knee
        ),
        None => println!("- balance: no density tried is enough"),
    }
}

fn print_failures(cure: &CureRow) {
    for kind in ["island", "floating", "speck", "torn"] {
        let found: Vec<String> = cure
            .failures
            .iter()
            .filter(|f| f.kind == kind)
            .take(5)
            .map(|f| {
                let unit = if kind == "torn" { "MPa" } else { "mm²" };
                format!(
                    "({:.1}, {:.1}, {:.1}) {:.2} {unit}",
                    f.at[0], f.at[1], f.at[2], f.value
                )
            })
            .collect();
        if !found.is_empty() {
            let all = cure.failures.iter().filter(|f| f.kind == kind).count();
            println!("  - {kind} ×{all}: {}", found.join(", "));
        }
    }
}

fn print_findings(shape: &Shape) {
    let order = [
        Kind::Hanging,
        Kind::Bare,
        Kind::Overloaded,
        Kind::RimGap,
        Kind::ThinTip,
        Kind::Visible,
        Kind::OnModel,
        Kind::Slender,
        Kind::Crowded,
    ];
    for kind in order {
        let mut found: Vec<_> = shape.findings.iter().filter(|f| f.kind == kind).collect();
        if found.is_empty() {
            continue;
        }
        found.sort_by(|a, b| b.value.total_cmp(&a.value));
        let shown: Vec<String> = found
            .iter()
            .take(5)
            .map(|f| {
                format!(
                    "({:.1}, {:.1}, {:.1}){}",
                    f.at[0],
                    f.at[1],
                    f.at[2],
                    value_of(kind, f.value)
                )
            })
            .collect();
        println!("  - {kind:?} ×{}: {}", found.len(), shown.join(", "));
    }
}

fn value_of(kind: Kind, value: f32) -> String {
    match kind {
        Kind::Bare => format!(" {value:.1} mm²"),
        Kind::Overloaded => format!(" uses {value:.1}× what it holds"),
        Kind::RimGap => format!(" {value:.1} mm from a tip"),
        Kind::ThinTip => format!(" on {value:.2} mm"),
        Kind::Slender => format!(" {value:.0}:1"),
        _ => String::new(),
    }
}

/// The summary table over every row.
pub fn print_table(report: &Report) {
    println!(
        "| model | profile | holds | enough | contacts | supports ml | tips/cm² | use p90 | over | rim mm | bare mm² | visible | thin | specks | knee |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for row in report.rows.iter().filter(|row| row.skipped.is_none()) {
        let shape = &row.shape;
        println!(
            "| {} | {} | {} | {} | {} | {:.2} | {:.1} | {:.2} | {} | {:.0} | {:.0} | {} | {} | {} | {} |",
            row.model,
            row.profile,
            if row.held() { "yes" } else { "**no**" },
            if row.enough() { "yes" } else { "**no**" },
            shape.standing,
            shape.supports_ml,
            row.per_cm2(),
            shape.use_p90,
            shape.overloaded,
            shape.rim_bare_mm,
            shape.bare_mm2,
            shape.visible,
            shape.thin_tips,
            row.cure.specks,
            row.knee()
                .map_or("-".to_owned(), |knee| format!("×{knee:.2}")),
        );
    }
    let counted = report.rows.iter().filter(|row| row.skipped.is_none());
    let (held, enough, all) = counted.fold((0, 0, 0), |(held, enough, all), row| {
        (
            held + usize::from(row.held()),
            enough + usize::from(row.enough()),
            all + 1,
        )
    });
    println!("\n{held} of {all} hold, {enough} with enough supports");
}

/// What got worse against `kept`, one line each.
pub fn regressions(kept: &Report, now: &Report) -> Vec<String> {
    let mut worse = Vec::new();
    for row in now.rows.iter().filter(|row| row.skipped.is_none()) {
        let Some(before) = kept.rows.iter().find(|before| {
            before.model == row.model && before.profile == row.profile && before.skipped.is_none()
        }) else {
            continue;
        };
        let name = format!("{} · {}", row.model, row.profile);
        if before.held() && !row.held() {
            worse.push(format!("{name}: held, now fails"));
        }
        if before.enough() && !row.enough() {
            worse.push(format!("{name}: had enough supports, now overloaded"));
        }
        let (b, n) = (&before.shape, &row.shape);
        if n.standing as f32 > b.standing as f32 * GROWTH + 2.0 {
            worse.push(format!("{name}: contacts {} → {}", b.standing, n.standing));
        }
        if n.supports_ml > b.supports_ml * GROWTH + 0.02 {
            worse.push(format!(
                "{name}: supports {:.2} → {:.2} ml",
                b.supports_ml, n.supports_ml
            ));
        }
        if n.visible > b.visible || n.thin_tips > b.thin_tips + 1 {
            worse.push(format!(
                "{name}: marks visible {} → {}, thin {} → {}",
                b.visible, n.visible, b.thin_tips, n.thin_tips
            ));
        }
    }
    worse
}
